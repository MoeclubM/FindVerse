use anyhow::Context;
use redis::Client as RedisClient;
use serde::Deserialize;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{collections::HashMap, path::PathBuf, time::Duration};
use tokio::fs;
use uuid::Uuid;

use crate::{
    auth_support::{
        PASSWORD_SCHEME_ARGON2ID, PASSWORD_SCHEME_LEGACY_SHA256_SALT_V1, hash_password,
    },
    config::Config,
    error::ApiError,
    store::developer::import_legacy_developer_store,
};

#[derive(Clone)]
pub struct DatabaseBackends {
    pub pg_pool: PgPool,
    pub redis_client: RedisClient,
}

impl DatabaseBackends {
    pub async fn connect(config: &Config) -> anyhow::Result<Self> {
        let pg_pool = PgPoolOptions::new()
            .max_connections(config.postgres_max_connections)
            .acquire_timeout(Duration::from_secs(config.postgres_acquire_timeout_secs))
            .connect(&config.postgres_url)
            .await
            .with_context(|| format!("failed to connect to postgres at {}", config.postgres_url))?;

        let redis_client = RedisClient::open(config.redis_url.clone())
            .with_context(|| format!("invalid redis url {}", config.redis_url))?;

        redis_client
            .get_multiplexed_async_connection()
            .await
            .context("failed to connect to redis")?;

        Ok(Self {
            pg_pool,
            redis_client,
        })
    }

    pub async fn prepare_control_plane(&self, config: &Config) -> Result<(), ApiError> {
        if config.bootstrap_admin_enabled {
            seed_bootstrap_admin(&self.pg_pool, config).await?;
        }
        import_legacy_auth_data(&self.pg_pool, &config.dev_auth_store_path).await?;
        import_legacy_developer_store(&self.pg_pool, &config.developer_store_path).await?;
        Ok(())
    }

    pub async fn ping_postgres(&self) -> bool {
        sqlx::query_scalar::<_, i32>("select 1")
            .fetch_one(&self.pg_pool)
            .await
            .is_ok()
    }

    pub async fn ping_redis(&self) -> bool {
        match self.redis_client.get_multiplexed_async_connection().await {
            Ok(mut conn) => redis::cmd("PING")
                .query_async::<String>(&mut conn)
                .await
                .map(|response| response.eq_ignore_ascii_case("PONG"))
                .unwrap_or(false),
            Err(_) => false,
        }
    }
}

#[derive(Debug, Deserialize)]
struct LegacyDevAuthState {
    #[serde(default)]
    accounts: HashMap<String, LegacyDevAccount>,
}

#[derive(Debug, Deserialize)]
struct LegacyDevAccount {
    user_id: String,
    username: String,
    password_hash: String,
    salt: String,
    created_at: chrono::DateTime<chrono::Utc>,
    #[serde(default = "default_true")]
    enabled: bool,
}

async fn import_legacy_auth_data(pg_pool: &PgPool, path: &PathBuf) -> Result<(), ApiError> {
    if fs::metadata(path).await.is_err() {
        return Ok(());
    }

    let raw = fs::read_to_string(path)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;
    let state: LegacyDevAuthState =
        serde_json::from_str(&raw).map_err(|error| ApiError::Internal(error.into()))?;

    for account in state.accounts.into_values() {
        import_legacy_account(pg_pool, account).await?;
    }

    Ok(())
}

async fn import_legacy_account(
    pg_pool: &PgPool,
    account: LegacyDevAccount,
) -> Result<(), ApiError> {
    let existing =
        sqlx::query_scalar::<_, i64>("select count(*) from users where external_id = $1")
            .bind(account.user_id.as_str())
            .fetch_one(pg_pool)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

    if existing > 0 {
        return Ok(());
    }

    let user_id = Uuid::now_v7();
    let mut tx = pg_pool
        .begin()
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

    sqlx::query(
        "insert into users (id, external_id, username, role, enabled, created_at) values ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_id)
    .bind(account.user_id.as_str())
    .bind(account.username)
    .bind("developer")
    .bind(account.enabled)
    .bind(account.created_at)
    .execute(&mut *tx)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?;

    sqlx::query(
        "insert into password_credentials (user_id, password_hash, password_scheme, password_salt, created_at, updated_at) values ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_id)
    .bind(account.password_hash)
    .bind(PASSWORD_SCHEME_LEGACY_SHA256_SALT_V1)
    .bind(account.salt)
    .bind(account.created_at)
    .bind(account.created_at)
    .execute(&mut *tx)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?;

    tx.commit()
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

    Ok(())
}

async fn seed_bootstrap_admin(pg_pool: &PgPool, config: &Config) -> Result<(), ApiError> {
    let existing =
        sqlx::query_scalar::<_, i64>("select count(*) from users where external_id = $1")
            .bind(format!("local:{}", config.local_admin_username))
            .fetch_one(pg_pool)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

    if existing > 0 {
        return Ok(());
    }

    let password_hash = hash_password(&config.local_admin_password)?;
    let user_id = Uuid::now_v7();

    let mut tx = pg_pool
        .begin()
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

    sqlx::query(
        "insert into users (id, external_id, username, role, enabled) values ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(format!("local:{}", config.local_admin_username))
    .bind(config.local_admin_username.as_str())
    .bind("admin")
    .bind(true)
    .execute(&mut *tx)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?;

    sqlx::query(
        "insert into password_credentials (user_id, password_hash, password_scheme, created_at, updated_at) values ($1, $2, $3, now(), now())",
    )
    .bind(user_id)
    .bind(password_hash)
    .bind(PASSWORD_SCHEME_ARGON2ID)
    .execute(&mut *tx)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?;

    tx.commit()
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

    Ok(())
}

fn default_true() -> bool {
    true
}
