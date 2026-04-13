use std::path::PathBuf;

use anyhow::Context;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::fs;
use uuid::Uuid;

use crate::{
    error::ApiError,
    models::{
        AdminUserRecord, ApiKeyMetadata, CreateKeyRequest, CreatedKeyResponse,
        DeveloperUsageResponse, UpdateUserRequest,
    },
};

use super::{bearer_hash, generate_token, hash_token};

#[derive(Debug, Clone)]
pub struct DeveloperStore {
    pg_pool: PgPool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct LegacyDeveloperStoreState {
    #[serde(default)]
    records: std::collections::HashMap<String, LegacyDeveloperRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyDeveloperRecord {
    developer_id: String,
    #[serde(default = "default_daily_limit")]
    daily_limit: u32,
    #[serde(default)]
    used_today: u32,
    #[serde(default = "default_usage_day")]
    usage_day: NaiveDate,
    #[serde(default)]
    keys: Vec<LegacyStoredKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyStoredKey {
    id: String,
    name: String,
    preview: String,
    token_hash: String,
    created_at: DateTime<Utc>,
    #[serde(default)]
    revoked_at: Option<DateTime<Utc>>,
}

impl DeveloperStore {
    pub fn new(pg_pool: PgPool) -> Self {
        Self { pg_pool }
    }

    pub async fn create_developer_key(
        &self,
        developer_id: &str,
        request: CreateKeyRequest,
    ) -> Result<CreatedKeyResponse, ApiError> {
        let clean_name = request.name.trim();
        if clean_name.len() < 2 {
            return Err(ApiError::BadRequest(
                "key name must contain at least 2 characters".to_string(),
            ));
        }

        let user = find_user_by_external_id(&self.pg_pool, developer_id).await?;
        let token = generate_token("fvk");
        let preview = format!("{}...{}", &token[..8], &token[token.len() - 4..]);
        let created_at = Utc::now();
        let id = Uuid::now_v7().to_string();

        sqlx::query(
            "insert into api_keys (id, user_id, name, preview, token_hash, created_at) values ($1, $2, $3, $4, $5, $6)",
        )
        .bind(Uuid::parse_str(&id).map_err(|error| ApiError::Internal(error.into()))?)
        .bind(user.id)
        .bind(clean_name)
        .bind(preview.as_str())
        .bind(hash_token(&token))
        .bind(created_at)
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(CreatedKeyResponse {
            id,
            name: clean_name.to_string(),
            preview,
            token,
            created_at,
        })
    }

    pub async fn list_developer_keys(
        &self,
        developer_id: &str,
    ) -> Result<DeveloperUsageResponse, ApiError> {
        self.developer_usage(developer_id).await
    }

    pub async fn revoke_developer_key(
        &self,
        developer_id: &str,
        key_id: &str,
    ) -> Result<(), ApiError> {
        let user = find_user_by_external_id(&self.pg_pool, developer_id).await?;
        let key_uuid = Uuid::parse_str(key_id)
            .map_err(|_| ApiError::NotFound("api key not found".to_string()))?;
        let updated = sqlx::query(
            "update api_keys set revoked_at = now() where id = $1 and user_id = $2 and revoked_at is null",
        )
        .bind(key_uuid)
        .bind(user.id)
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .rows_affected();

        if updated == 0 {
            let exists = sqlx::query_scalar::<_, i64>(
                "select count(*) from api_keys where id = $1 and user_id = $2",
            )
            .bind(key_uuid)
            .bind(user.id)
            .fetch_one(&self.pg_pool)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;
            if exists > 0 {
                return Err(ApiError::Conflict("api key already revoked".to_string()));
            }
            return Err(ApiError::NotFound("api key not found".to_string()));
        }

        Ok(())
    }

    pub async fn developer_usage(
        &self,
        developer_id: &str,
    ) -> Result<DeveloperUsageResponse, ApiError> {
        let user = find_user_by_external_id(&self.pg_pool, developer_id).await?;
        ensure_daily_usage_row(&self.pg_pool, user.id).await?;
        let user = find_user_by_external_id(&self.pg_pool, developer_id).await?;
        build_usage_response(&self.pg_pool, &user).await
    }

    pub async fn validate_and_track_developer_key(
        &self,
        auth_header: Option<&str>,
    ) -> Result<(), ApiError> {
        let Some(header) = auth_header else {
            return Err(ApiError::Unauthorized("api key required".to_string()));
        };

        let token_hash = bearer_hash(header)?;
        let tracking = sqlx::query_as::<_, KeyTrackingRow>(
            "with key_owner as (
                 select u.id as user_id, u.daily_limit
                 from api_keys k
                 join users u on u.id = k.user_id
                 where k.token_hash = $1 and k.revoked_at is null and u.enabled = true and u.role in ('developer', 'admin')
             ),
             usage_seed as (
                 insert into daily_usage (user_id, usage_day, used_count)
                 select user_id, current_date, 0
                 from key_owner
                 on conflict (user_id, usage_day) do nothing
             ),
             incremented as (
                 update daily_usage du
                 set used_count = du.used_count + 1
                 from key_owner ko
                 where du.user_id = ko.user_id
                   and du.usage_day = current_date
                   and du.used_count < ko.daily_limit
                 returning du.user_id
             ),
             touch_key as (
                 update api_keys
                 set last_used_at = now()
                 where token_hash = $1 and exists (select 1 from incremented)
                 returning id
             )
             select
                 exists(select 1 from key_owner) as key_found,
                 exists(select 1 from incremented) as counted",
        )
        .bind(token_hash)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        if !tracking.key_found {
            return Err(ApiError::Unauthorized("invalid api key".to_string()));
        }
        if !tracking.counted {
            return Err(ApiError::TooManyRequests(
                "daily request quota exceeded".to_string(),
            ));
        }

        Ok(())
    }

    pub async fn list_all_user_usage(&self) -> Result<Vec<DeveloperUsageResponse>, ApiError> {
        let users = sqlx::query_as::<_, UsageUserRow>(
            "select u.id, u.external_id, u.daily_limit, coalesce(du.used_count, 0) as used_today
             from users u
             left join daily_usage du on du.user_id = u.id and du.usage_day = current_date
             where u.role in ('developer', 'admin')
             order by u.created_at asc",
        )
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        let mut result = Vec::with_capacity(users.len());
        for user in users {
            result.push(build_usage_response(&self.pg_pool, &user).await?);
        }

        Ok(result)
    }

    pub async fn update_user_quota(
        &self,
        developer_id: &str,
        request: UpdateUserRequest,
    ) -> Result<(), ApiError> {
        let daily_limit = request.daily_limit.map(|value| value.max(1) as i32);
        let updated = sqlx::query(
            "update users
             set daily_limit = coalesce($2, daily_limit)
             where external_id = $1 and role in ('developer', 'admin')",
        )
        .bind(developer_id)
        .bind(daily_limit)
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .rows_affected();
        if updated == 0 {
            return Err(ApiError::NotFound("user not found".to_string()));
        }
        Ok(())
    }

    pub fn build_admin_user_record(
        usage: &DeveloperUsageResponse,
        username: &str,
        role: &str,
        enabled: bool,
        created_at: chrono::DateTime<Utc>,
    ) -> AdminUserRecord {
        AdminUserRecord {
            user_id: usage.developer_id.clone(),
            username: username.to_string(),
            role: role.to_string(),
            enabled,
            created_at,
            daily_limit: usage.daily_limit,
            used_today: usage.used_today,
            key_count: usage.keys.len(),
        }
    }
}

pub async fn import_legacy_developer_store(
    pg_pool: &PgPool,
    path: &PathBuf,
) -> Result<(), ApiError> {
    if fs::metadata(path).await.is_err() {
        return Ok(());
    }

    let raw = fs::read_to_string(path)
        .await
        .context("failed to read developer store file")
        .map_err(ApiError::Internal)?;
    let state: LegacyDeveloperStoreState = serde_json::from_str(&raw)
        .context("failed to parse developer store file")
        .map_err(ApiError::Internal)?;

    for record in state.records.into_values() {
        import_legacy_record(pg_pool, record).await?;
    }

    Ok(())
}

async fn import_legacy_record(
    pg_pool: &PgPool,
    record: LegacyDeveloperRecord,
) -> Result<(), ApiError> {
    let user = ensure_developer_usage_record(pg_pool, &record.developer_id).await?;

    sqlx::query("update users set daily_limit = $2 where id = $1")
        .bind(user.id)
        .bind(record.daily_limit as i32)
        .execute(pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

    sqlx::query(
        "insert into daily_usage (user_id, usage_day, used_count)
         values ($1, $2, $3)
         on conflict (user_id, usage_day)
         do update set used_count = excluded.used_count",
    )
    .bind(user.id)
    .bind(record.usage_day)
    .bind(record.used_today as i32)
    .execute(pg_pool)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?;

    for key in record.keys {
        let key_uuid = Uuid::parse_str(&key.id).unwrap_or_else(|_| Uuid::now_v7());
        sqlx::query(
            "insert into api_keys (id, user_id, name, preview, token_hash, created_at, revoked_at)
             values ($1, $2, $3, $4, $5, $6, $7)
             on conflict (id) do nothing",
        )
        .bind(key_uuid)
        .bind(user.id)
        .bind(key.name)
        .bind(key.preview)
        .bind(key.token_hash)
        .bind(key.created_at)
        .bind(key.revoked_at)
        .execute(pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;
    }

    Ok(())
}

#[derive(sqlx::FromRow, Clone)]
struct UsageUserRow {
    id: Uuid,
    external_id: String,
    daily_limit: i32,
    used_today: i32,
}

#[derive(sqlx::FromRow)]
struct KeyTrackingRow {
    key_found: bool,
    counted: bool,
}

#[derive(sqlx::FromRow)]
struct StoredApiKeyRow {
    id: Uuid,
    name: String,
    preview: String,
    created_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

async fn build_usage_response(
    pg_pool: &PgPool,
    user: &UsageUserRow,
) -> Result<DeveloperUsageResponse, ApiError> {
    let keys = sqlx::query_as::<_, StoredApiKeyRow>(
        "select id, name, preview, created_at, revoked_at from api_keys where user_id = $1 order by created_at asc",
    )
    .bind(user.id)
    .fetch_all(pg_pool)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?;

    Ok(DeveloperUsageResponse {
        developer_id: user.external_id.clone(),
        daily_limit: user.daily_limit.max(1) as u32,
        used_today: user.used_today.max(0) as u32,
        keys: keys
            .into_iter()
            .map(|key| ApiKeyMetadata {
                id: key.id.to_string(),
                name: key.name,
                preview: key.preview,
                created_at: key.created_at,
                revoked_at: key.revoked_at,
            })
            .collect(),
    })
}

async fn find_user_by_external_id(
    pg_pool: &PgPool,
    developer_id: &str,
) -> Result<UsageUserRow, ApiError> {
    sqlx::query_as::<_, UsageUserRow>(
        "select u.id, u.external_id, u.daily_limit, coalesce(du.used_count, 0) as used_today
         from users u
         left join daily_usage du on du.user_id = u.id and du.usage_day = current_date
         where u.external_id = $1 and u.role in ('developer', 'admin')",
    )
    .bind(developer_id)
    .fetch_optional(pg_pool)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?
    .ok_or_else(|| ApiError::NotFound("user record not found".to_string()))
}

async fn ensure_daily_usage_row(pg_pool: &PgPool, user_id: Uuid) -> Result<(), ApiError> {
    sqlx::query(
        "insert into daily_usage (user_id, usage_day, used_count)
         values ($1, current_date, 0)
         on conflict (user_id, usage_day) do nothing",
    )
    .bind(user_id)
    .execute(pg_pool)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?;

    Ok(())
}

async fn ensure_developer_usage_record(
    pg_pool: &PgPool,
    developer_id: &str,
) -> Result<UsageUserRow, ApiError> {
    let Some(user) = sqlx::query_as::<_, UsageUserRow>(
        "select u.id, u.external_id, u.daily_limit, coalesce(du.used_count, 0) as used_today
         from users u
         left join daily_usage du on du.user_id = u.id and du.usage_day = current_date
         where u.external_id = $1 and u.role in ('developer', 'admin')",
    )
    .bind(developer_id)
    .fetch_optional(pg_pool)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?
    else {
        return Err(ApiError::NotFound("user record not found".to_string()));
    };

    ensure_daily_usage_row(pg_pool, user.id).await?;

    find_user_by_external_id(pg_pool, developer_id).await
}

fn default_daily_limit() -> u32 {
    10_000
}

fn default_usage_day() -> NaiveDate {
    Utc::now().date_naive()
}
