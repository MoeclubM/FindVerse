use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::{
    auth_support::{PASSWORD_SCHEME_ARGON2ID, bearer_token, verify_password},
    error::ApiError,
    models::{AdminLoginRequest, AdminSessionResponse},
    store::{generate_token, hash_token},
};

#[derive(Debug, Clone)]
pub struct AdminAuth {
    pg_pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct AdminIdentity {
    pub user_id: String,
}

impl AdminAuth {
    pub fn new(pg_pool: PgPool) -> Self {
        Self { pg_pool }
    }

    pub async fn login(
        &self,
        request: AdminLoginRequest,
    ) -> Result<AdminSessionResponse, ApiError> {
        let username = request.username.trim();
        let record = sqlx::query_as::<_, AdminPasswordRow>(
            "select u.id, u.external_id, u.username, u.enabled, pc.password_hash, pc.password_scheme
             from users u
             join password_credentials pc on pc.user_id = u.id
             where u.username = $1 and u.role = 'admin'",
        )
        .bind(username)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .ok_or_else(|| ApiError::Unauthorized("invalid username or password".to_string()))?;

        if !record.enabled {
            return Err(ApiError::Unauthorized("account is disabled".to_string()));
        }
        if record.password_scheme != PASSWORD_SCHEME_ARGON2ID {
            return Err(ApiError::Conflict(
                "admin password credential must be migrated before login".to_string(),
            ));
        }
        if !verify_password(&request.password, &record.password_hash)? {
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

        let response = create_admin_session_tx(
            &mut tx,
            record.id,
            &record.external_id,
            &record.username,
            now,
        )
        .await?;

        tx.commit()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(response)
    }

    pub async fn authorize(&self, auth_header: Option<&str>) -> Result<AdminIdentity, ApiError> {
        let token = bearer_token(auth_header)?;
        let session = authorize_admin_session(&self.pg_pool, token).await?;
        Ok(AdminIdentity {
            user_id: session.external_id,
        })
    }

    pub async fn current_session(
        &self,
        auth_header: Option<&str>,
    ) -> Result<AdminSessionResponse, ApiError> {
        let token = bearer_token(auth_header)?;
        let session = authorize_admin_session(&self.pg_pool, token).await?;
        Ok(AdminSessionResponse {
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
            return Err(ApiError::Unauthorized(
                "admin session is invalid".to_string(),
            ));
        }

        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct AdminPasswordRow {
    id: uuid::Uuid,
    external_id: String,
    username: String,
    enabled: bool,
    password_hash: String,
    password_scheme: String,
}

#[derive(sqlx::FromRow)]
struct AuthorizedAdminSessionRow {
    external_id: String,
    username: String,
}

async fn create_admin_session_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: uuid::Uuid,
    external_id: &str,
    username: &str,
    now: DateTime<Utc>,
) -> Result<AdminSessionResponse, ApiError> {
    let token = generate_token("fva");
    sqlx::query(
        "insert into sessions (id, user_id, token_hash, created_at, last_used_at) values ($1, $2, $3, $4, $4)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(user_id)
    .bind(hash_token(&token))
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?;

    Ok(AdminSessionResponse {
        user_id: external_id.to_string(),
        username: username.to_string(),
        token,
    })
}

async fn authorize_admin_session(
    pg_pool: &PgPool,
    token: &str,
) -> Result<AuthorizedAdminSessionRow, ApiError> {
    let token_hash = hash_token(token);
    let session = sqlx::query_as::<_, AuthorizedAdminSessionRow>(
        "select u.external_id, u.username
         from sessions s
         join users u on u.id = s.user_id
         where s.token_hash = $1 and s.revoked_at is null and u.enabled = true and u.role = 'admin'",
    )
    .bind(&token_hash)
    .fetch_optional(pg_pool)
    .await
    .map_err(|error| ApiError::Internal(error.into()))?
    .ok_or_else(|| ApiError::Unauthorized("admin session is invalid".to_string()))?;

    sqlx::query("update sessions set last_used_at = now() where token_hash = $1")
        .bind(token_hash)
        .execute(pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

    Ok(session)
}
