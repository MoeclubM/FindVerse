use anyhow::Context;
use redis::Client as RedisClient;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use uuid::Uuid;

use crate::{
    auth_support::{PASSWORD_SCHEME_ARGON2ID, hash_password},
    config::Config,
    error::ApiError,
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
