use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::{
    auth_support::{PASSWORD_SCHEME_ARGON2ID, bearer_token, hash_password, verify_password},
    error::ApiError,
    models::{DevLoginRequest, DevRegisterRequest, DevSessionResponse},
    store::{generate_token, hash_token},
};

#[derive(Debug, Clone)]
pub struct DevAuthStore {
    pg_pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct DevAccount {
    pub user_id: String,
    pub username: String,
    pub created_at: DateTime<Utc>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct DevUserIdentity {
    pub user_id: String,
}

impl DevAuthStore {
    pub fn new(pg_pool: PgPool) -> Self {
        Self { pg_pool }
    }

    pub async fn register(
        &self,
        request: DevRegisterRequest,
    ) -> Result<DevSessionResponse, ApiError> {
        let username = request.username.trim().to_lowercase();
        if username.len() < 3
            || username
                .chars()
                .any(|c| !c.is_alphanumeric() && c != '_' && c != '-')
        {
            return Err(ApiError::BadRequest(
                "username must be 3+ alphanumeric characters (_, - allowed)".to_string(),
            ));
        }
        if request.password.len() < 8 {
            return Err(ApiError::BadRequest(
                "password must be at least 8 characters".to_string(),
            ));
        }

        let existing = sqlx::query_scalar::<_, i64>(
            "select count(*) from users where username = $1 and role = 'developer'",
        )
        .bind(username.as_str())
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;
        if existing > 0 {
            return Err(ApiError::Conflict("username already taken".to_string()));
        }

        let user_uuid = uuid::Uuid::now_v7();
        let user_id = format!("dev:{user_uuid}");
        let password_hash = hash_password(&request.password)?;
        let created_at = Utc::now();

        let mut tx = self
            .pg_pool
            .begin()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        sqlx::query(
            "insert into users (id, external_id, username, role, enabled, created_at) values ($1, $2, $3, 'developer', true, $4)",
        )
        .bind(user_uuid)
        .bind(user_id.as_str())
        .bind(username.as_str())
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        sqlx::query(
            "insert into password_credentials (user_id, password_hash, password_scheme, created_at, updated_at) values ($1, $2, $3, $4, $4)",
        )
        .bind(user_uuid)
        .bind(password_hash)
        .bind(PASSWORD_SCHEME_ARGON2ID)
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        let session =
            create_session_tx(&mut tx, user_uuid, &user_id, &username, "fvs", created_at).await?;

        tx.commit()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(session)
    }

    pub async fn login(&self, request: DevLoginRequest) -> Result<DevSessionResponse, ApiError> {
        let username = request.username.trim().to_lowercase();
        let record = sqlx::query_as::<_, UserPasswordRow>(
            "select u.id, u.external_id, u.username, u.enabled, pc.password_hash, pc.password_scheme, pc.password_salt
             from users u
             join password_credentials pc on pc.user_id = u.id
             where u.username = $1 and u.role = 'developer'",
        )
        .bind(username.as_str())
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .ok_or_else(|| ApiError::Unauthorized("invalid username or password".to_string()))?;

        if !record.enabled {
            return Err(ApiError::Unauthorized("account is disabled".to_string()));
        }
        if !verify_password(
            &request.password,
            &record.password_hash,
            &record.password_scheme,
            record.password_salt.as_deref(),
        )? {
            return Err(ApiError::Unauthorized(
                "invalid username or password".to_string(),
            ));
        }

        let now = Utc::now();
        let mut tx = self
            .pg_pool
            .begin()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        maybe_upgrade_password_tx(&mut tx, &record, &request.password, now).await?;
        let session = create_session_tx(
            &mut tx,
            record.id,
            &record.external_id,
            &record.username,
            "fvs",
            now,
        )
        .await?;

        tx.commit()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(session)
    }

    pub async fn authorize(&self, auth_header: Option<&str>) -> Result<DevUserIdentity, ApiError> {
        let token = bearer_token(auth_header)?;
        let session = authorize_session(&self.pg_pool, token, "developer").await?;
        Ok(DevUserIdentity {
            user_id: session.external_id,
        })
    }

    pub async fn current_session(
        &self,
        auth_header: Option<&str>,
    ) -> Result<DevSessionResponse, ApiError> {
        let token = bearer_token(auth_header)?;
        let session = authorize_session(&self.pg_pool, token, "developer").await?;
        Ok(DevSessionResponse {
            user_id: session.external_id,
            username: session.username,
            token: token.to_string(),
        })
    }

    pub async fn logout(&self, auth_header: Option<&str>) -> Result<(), ApiError> {
        let token = bearer_token(auth_header)?;
        let revoked = sqlx::query(
            "update sessions set revoked_at = now() where token_hash = $1 and revoked_at is null",
        )
        .bind(hash_token(token))
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .rows_affected();
        if revoked == 0 {
            return Err(ApiError::Unauthorized("invalid session".to_string()));
        }
        Ok(())
    }

    pub async fn list_accounts(&self) -> Vec<DevAccount> {
        match sqlx::query_as::<_, DevAccountRow>(
            "select external_id, username, created_at, enabled from users where role = 'developer' order by created_at asc",
        )
        .fetch_all(&self.pg_pool)
        .await
        {
            Ok(rows) => rows
                .into_iter()
                .map(|row| DevAccount {
                    user_id: row.external_id,
                    username: row.username,
                    created_at: row.created_at,
                    enabled: row.enabled,
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    pub async fn set_enabled(&self, user_id: &str, enabled: bool) -> Result<(), ApiError> {
        let mut tx = self
            .pg_pool
            .begin()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        let updated = sqlx::query(
            "update users set enabled = $2 where external_id = $1 and role = 'developer'",
        )
        .bind(user_id)
        .bind(enabled)
        .execute(&mut *tx)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .rows_affected();
        if updated == 0 {
            return Err(ApiError::NotFound("developer not found".to_string()));
        }

        if !enabled {
            sqlx::query(
                "update sessions set revoked_at = now() where user_id = (select id from users where external_id = $1) and revoked_at is null",
            )
            .bind(user_id)
            .execute(&mut *tx)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;
        }

        tx.commit()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;
        Ok(())
    }

    pub async fn update_password(&self, user_id: &str, password: &str) -> Result<(), ApiError> {
        if password.len() < 8 {
            return Err(ApiError::BadRequest(
                "password must be at least 8 characters".to_string(),
            ));
        }

        let user_uuid = sqlx::query_scalar::<_, uuid::Uuid>(
            "select id from users where external_id = $1 and role = 'developer'",
        )
        .bind(user_id)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .ok_or_else(|| ApiError::NotFound("developer not found".to_string()))?;

        let updated = sqlx::query(
            "update password_credentials
             set password_hash = $2, password_scheme = $3, password_salt = null, updated_at = $4
             where user_id = $1",
        )
        .bind(user_uuid)
        .bind(hash_password(password)?)
        .bind(PASSWORD_SCHEME_ARGON2ID)
        .bind(Utc::now())
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .rows_affected();
        if updated == 0 {
            return Err(ApiError::NotFound("developer not found".to_string()));
        }

        Ok(())
    }

    pub async fn delete_account(&self, user_id: &str) -> Result<(), ApiError> {
        let deleted =
            sqlx::query("delete from users where external_id = $1 and role = 'developer'")
                .bind(user_id)
                .execute(&self.pg_pool)
                .await
                .map_err(|error| ApiError::Internal(error.into()))?
                .rows_affected();
        if deleted == 0 {
            return Err(ApiError::NotFound("developer not found".to_string()));
        }
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct UserPasswordRow {
    id: uuid::Uuid,
    external_id: String,
    username: String,
    enabled: bool,
    password_hash: String,
    password_scheme: String,
    password_salt: Option<String>,
}

#[derive(sqlx::FromRow)]
struct AuthorizedSessionRow {
    external_id: String,
    username: String,
}

#[derive(sqlx::FromRow)]
struct DevAccountRow {
    external_id: String,
    username: String,
    created_at: DateTime<Utc>,
    enabled: bool,
}

async fn maybe_upgrade_password_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    record: &UserPasswordRow,
    password: &str,
    now: DateTime<Utc>,
) -> Result<(), ApiError> {
    if record.password_scheme == PASSWORD_SCHEME_ARGON2ID {
        return Ok(());
    }

    let upgraded_hash = hash_password(password)?;
    sqlx::query(
        "update password_credentials set password_hash = $2, password_scheme = $3, password_salt = null, updated_at = $4 where user_id = $1",
    )
    .bind(record.id)
    .bind(upgraded_hash)
    .bind(PASSWORD_SCHEME_ARGON2ID)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?;
    Ok(())
}

async fn create_session_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_uuid: uuid::Uuid,
    external_id: &str,
    username: &str,
    prefix: &str,
    now: DateTime<Utc>,
) -> Result<DevSessionResponse, ApiError> {
    let token = generate_token(prefix);
    sqlx::query(
        "insert into sessions (id, user_id, token_hash, created_at, last_used_at) values ($1, $2, $3, $4, $4)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(user_uuid)
    .bind(hash_token(&token))
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?;

    Ok(DevSessionResponse {
        user_id: external_id.to_string(),
        username: username.to_string(),
        token,
    })
}

async fn authorize_session(
    pg_pool: &PgPool,
    token: &str,
    role: &str,
) -> Result<AuthorizedSessionRow, ApiError> {
    let session = sqlx::query_as::<_, AuthorizedSessionRow>(
        "select u.external_id, u.username
         from sessions s
         join users u on u.id = s.user_id
         where s.token_hash = $1 and s.revoked_at is null and u.enabled = true and u.role = $2",
    )
    .bind(hash_token(token))
    .bind(role)
    .fetch_optional(pg_pool)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?
    .ok_or_else(|| ApiError::Unauthorized("invalid session".to_string()))?;

    sqlx::query("update sessions set last_used_at = now() where token_hash = $1")
        .bind(hash_token(token))
        .execute(pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

    Ok(session)
}
