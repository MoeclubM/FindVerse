use std::{collections::HashMap, path::PathBuf, time::Duration};

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::fs;
use uuid::Uuid;

use crate::{
    auth_support::{PASSWORD_SCHEME_ARGON2ID, hash_password},
    blob_store::BlobStore,
    models::CrawlResultInput,
    store::{developer::import_legacy_developer_store, generate_token},
};

#[derive(Debug, Clone)]
pub struct LegacyMigrationConfig {
    pub postgres_url: String,
    pub postgres_max_connections: u32,
    pub postgres_acquire_timeout_secs: u64,
    pub blob_storage_url: Option<String>,
    pub dev_auth_store_path: Option<PathBuf>,
    pub developer_store_path: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct LegacyMigrationSummary {
    pub developer_store_imported: bool,
    pub auth_file_imported: bool,
    pub imported_accounts: usize,
    pub rotated_credentials: usize,
    pub skipped_legacy_sessions: usize,
    pub document_text_blobs_backfilled: usize,
    pub crawl_result_blobs_backfilled: usize,
    pub temporary_credentials: Vec<TemporaryCredential>,
}

#[derive(Debug, Serialize)]
pub struct TemporaryCredential {
    pub user_id: String,
    pub username: String,
    pub temporary_password: String,
}

pub async fn migrate_legacy_control_plane_data(
    config: LegacyMigrationConfig,
) -> anyhow::Result<LegacyMigrationSummary> {
    let pg_pool = PgPoolOptions::new()
        .max_connections(config.postgres_max_connections)
        .acquire_timeout(Duration::from_secs(config.postgres_acquire_timeout_secs))
        .connect(&config.postgres_url)
        .await
        .with_context(|| format!("failed to connect to postgres at {}", config.postgres_url))?;

    let mut summary = LegacyMigrationSummary {
        developer_store_imported: false,
        auth_file_imported: false,
        imported_accounts: 0,
        rotated_credentials: 0,
        skipped_legacy_sessions: 0,
        document_text_blobs_backfilled: 0,
        crawl_result_blobs_backfilled: 0,
        temporary_credentials: Vec::new(),
    };

    if let Some(path) = config.dev_auth_store_path.as_ref() {
        if fs::metadata(path).await.is_ok() {
            let result = import_legacy_auth_data(&pg_pool, path).await?;
            summary.auth_file_imported = true;
            summary.imported_accounts = result.imported_accounts;
            summary.skipped_legacy_sessions = result.skipped_sessions;
            summary
                .temporary_credentials
                .extend(result.temporary_credentials);
        }
    }

    if let Some(path) = config.developer_store_path.as_ref() {
        if fs::metadata(path).await.is_ok() {
            import_legacy_developer_store(&pg_pool, path)
                .await
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            summary.developer_store_imported = true;
        }
    }

    let rotated = rotate_non_argon_credentials(&pg_pool).await?;
    summary.rotated_credentials = rotated.len();
    summary.temporary_credentials.extend(rotated);

    if let Some(blob_storage_url) = config.blob_storage_url {
        let blob_store = BlobStore::new(pg_pool.clone(), blob_storage_url);
        let blob_summary = backfill_blob_storage(&pg_pool, &blob_store).await?;
        summary.document_text_blobs_backfilled = blob_summary.document_text_blobs_backfilled;
        summary.crawl_result_blobs_backfilled = blob_summary.crawl_result_blobs_backfilled;
    }

    Ok(summary)
}

#[derive(Debug, Default, Serialize)]
pub struct BlobBackfillSummary {
    pub document_text_blobs_backfilled: usize,
    pub crawl_result_blobs_backfilled: usize,
}

pub async fn backfill_blob_storage(
    pg_pool: &PgPool,
    blob_store: &BlobStore,
) -> anyhow::Result<BlobBackfillSummary> {
    let mut summary = BlobBackfillSummary::default();

    loop {
        let rows = sqlx::query_as::<_, LegacyDocumentBlobRow>(
            "select id, body
             from documents
             where text_blob_key is null
             order by id
             limit 128",
        )
        .fetch_all(pg_pool)
        .await?;

        if rows.is_empty() {
            break;
        }

        for row in rows {
            let blob_key = format!("documents/{}.txt", row.id);
            blob_store
                .write_text_blob(&blob_key, &row.body)
                .await
                .map_err(anyhow::Error::from)?;

            let updated = sqlx::query(
                "update documents
                 set text_blob_key = $2
                 where id = $1
                   and text_blob_key is null",
            )
            .bind(&row.id)
            .bind(&blob_key)
            .execute(pg_pool)
            .await?
            .rows_affected();

            summary.document_text_blobs_backfilled += updated as usize;
        }
    }

    loop {
        let rows = sqlx::query_as::<_, LegacyCrawlResultBlobRow>(
            "select id, lease_id, crawl_job_id, payload
             from crawl_result_blobs
             where blob_key is null
             order by created_at, id
             limit 128",
        )
        .fetch_all(pg_pool)
        .await?;

        if rows.is_empty() {
            break;
        }

        for row in rows {
            let result: CrawlResultInput = serde_json::from_value(row.payload.clone())
                .with_context(|| {
                    format!("crawl_result_blobs {} contains invalid payload", row.id)
                })?;
            let body = serde_json::to_vec(&result)?;
            let body_len = body.len() as i64;
            let blob_key = format!("crawl-results/{}/{}.json", row.lease_id, row.crawl_job_id);
            blob_store
                .write_blob_bytes(&blob_key, body, "application/json")
                .await
                .map_err(anyhow::Error::from)?;

            let updated = sqlx::query(
                "update crawl_result_blobs
                 set blob_key = $2,
                     blob_size_bytes = $3,
                     blob_content_type = $4,
                     payload = $5
                 where id = $1
                   and blob_key is null",
            )
            .bind(&row.id)
            .bind(&blob_key)
            .bind(body_len)
            .bind("application/json")
            .bind(serde_json::json!({}))
            .execute(pg_pool)
            .await?
            .rows_affected();

            summary.crawl_result_blobs_backfilled += updated as usize;
        }
    }

    let remaining_document_blobs: i64 =
        sqlx::query_scalar("select count(*) from documents where text_blob_key is null")
            .fetch_one(pg_pool)
            .await?;
    if remaining_document_blobs > 0 {
        anyhow::bail!(
            "document blob migration incomplete: {remaining_document_blobs} rows still missing text_blob_key"
        );
    }

    let remaining_result_blobs: i64 =
        sqlx::query_scalar("select count(*) from crawl_result_blobs where blob_key is null")
            .fetch_one(pg_pool)
            .await?;
    if remaining_result_blobs > 0 {
        anyhow::bail!(
            "crawl result blob migration incomplete: {remaining_result_blobs} rows still missing blob_key"
        );
    }

    Ok(summary)
}

#[derive(Debug, Deserialize)]
struct LegacyDevAuthState {
    #[serde(default)]
    accounts: HashMap<String, LegacyDevAccount>,
    #[serde(default)]
    sessions: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct LegacyDevAccount {
    user_id: String,
    username: String,
    created_at: DateTime<Utc>,
    #[serde(default = "default_true")]
    enabled: bool,
}

struct LegacyAuthImportResult {
    imported_accounts: usize,
    skipped_sessions: usize,
    temporary_credentials: Vec<TemporaryCredential>,
}

async fn import_legacy_auth_data(
    pg_pool: &PgPool,
    path: &PathBuf,
) -> anyhow::Result<LegacyAuthImportResult> {
    let raw = fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read legacy auth store {}", path.display()))?;
    let state: LegacyDevAuthState = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse legacy auth store {}", path.display()))?;
    let imported_accounts = state.accounts.len();

    let mut temporary_credentials = Vec::new();
    for account in state.accounts.into_values() {
        if let Some(credential) = upsert_legacy_account(pg_pool, account).await? {
            temporary_credentials.push(credential);
        }
    }

    Ok(LegacyAuthImportResult {
        imported_accounts,
        skipped_sessions: state.sessions.len(),
        temporary_credentials,
    })
}

async fn upsert_legacy_account(
    pg_pool: &PgPool,
    account: LegacyDevAccount,
) -> anyhow::Result<Option<TemporaryCredential>> {
    let mut tx = pg_pool.begin().await?;
    let existing = sqlx::query_as::<_, ExistingLegacyUserRow>(
        "select u.id, pc.password_scheme as credential_scheme
         from users u
         left join password_credentials pc on pc.user_id = u.id
         where u.external_id = $1 and u.role = 'developer'",
    )
    .bind(account.user_id.as_str())
    .fetch_optional(&mut *tx)
    .await?;

    let user_id = existing
        .as_ref()
        .map(|row| row.id)
        .unwrap_or_else(Uuid::now_v7);
    if existing.is_some() {
        sqlx::query(
            "update users
             set username = $2,
                 enabled = $3
             where id = $1",
        )
        .bind(user_id)
        .bind(account.username.as_str())
        .bind(account.enabled)
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query(
            "insert into users (id, external_id, username, role, enabled, created_at)
             values ($1, $2, $3, 'developer', $4, $5)",
        )
        .bind(user_id)
        .bind(account.user_id.as_str())
        .bind(account.username.as_str())
        .bind(account.enabled)
        .bind(account.created_at)
        .execute(&mut *tx)
        .await?;
    }

    let credential = if existing
        .as_ref()
        .and_then(|row| row.credential_scheme.as_deref())
        == Some(PASSWORD_SCHEME_ARGON2ID)
    {
        None
    } else {
        let temporary_password = generate_token("fvm");
        let password_hash = hash_password(&temporary_password)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        if existing
            .as_ref()
            .and_then(|row| row.credential_scheme.as_deref())
            .is_some()
        {
            sqlx::query(
                "update password_credentials
                 set password_hash = $2,
                     password_scheme = $3,
                     password_salt = null,
                     updated_at = now()
                 where user_id = $1",
            )
            .bind(user_id)
            .bind(password_hash)
            .bind(PASSWORD_SCHEME_ARGON2ID)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                "insert into password_credentials (
                    user_id,
                    password_hash,
                    password_scheme,
                    created_at,
                    updated_at
                 ) values ($1, $2, $3, $4, $4)",
            )
            .bind(user_id)
            .bind(password_hash)
            .bind(PASSWORD_SCHEME_ARGON2ID)
            .bind(account.created_at)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            "update sessions set revoked_at = now() where user_id = $1 and revoked_at is null",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        Some(TemporaryCredential {
            user_id: account.user_id,
            username: account.username,
            temporary_password,
        })
    };

    tx.commit().await?;
    Ok(credential)
}

async fn rotate_non_argon_credentials(
    pg_pool: &PgPool,
) -> anyhow::Result<Vec<TemporaryCredential>> {
    let rows = sqlx::query_as::<_, LegacyCredentialRow>(
        "select u.id, u.external_id, u.username
         from password_credentials pc
         join users u on u.id = pc.user_id
         where pc.password_scheme <> $1
         order by u.created_at asc",
    )
    .bind(PASSWORD_SCHEME_ARGON2ID)
    .fetch_all(pg_pool)
    .await?;

    let mut rotated = Vec::with_capacity(rows.len());
    for row in rows {
        let temporary_password = generate_token("fvm");
        let password_hash = hash_password(&temporary_password)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;

        let mut tx = pg_pool.begin().await?;
        sqlx::query(
            "update password_credentials
             set password_hash = $2,
                 password_scheme = $3,
                 password_salt = null,
                 updated_at = now()
             where user_id = $1",
        )
        .bind(row.id)
        .bind(password_hash)
        .bind(PASSWORD_SCHEME_ARGON2ID)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "update sessions set revoked_at = now() where user_id = $1 and revoked_at is null",
        )
        .bind(row.id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        rotated.push(TemporaryCredential {
            user_id: row.external_id,
            username: row.username,
            temporary_password,
        });
    }

    Ok(rotated)
}

#[derive(sqlx::FromRow)]
struct ExistingLegacyUserRow {
    id: Uuid,
    credential_scheme: Option<String>,
}

#[derive(sqlx::FromRow)]
struct LegacyCredentialRow {
    id: Uuid,
    external_id: String,
    username: String,
}

#[derive(sqlx::FromRow)]
struct LegacyDocumentBlobRow {
    id: String,
    body: String,
}

#[derive(sqlx::FromRow)]
struct LegacyCrawlResultBlobRow {
    id: String,
    lease_id: String,
    crawl_job_id: String,
    payload: serde_json::Value,
}

fn default_true() -> bool {
    true
}
