use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::{
    auth_support::{
        PASSWORD_SCHEME_ARGON2ID, USER_ROLE_ADMIN, USER_ROLE_DEVELOPER, bearer_token,
        hash_password, normalize_user_role, normalize_username, validate_password,
        verify_password,
    },
    error::ApiError,
    models::{
        CreateUserRequest, UpdateUserRequest, UserLoginRequest, UserRegisterRequest,
        UserSessionResponse,
    },
    store::{generate_token, hash_token},
};

#[derive(Debug, Clone)]
pub struct DevAuthStore {
    pg_pool: PgPool,
}

#[derive(Debug, Clone)]
pub struct UserAccount {
    pub user_id: String,
    pub username: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct UserIdentity {
    pub user_id: String,
    pub username: String,
    pub role: String,
}

impl DevAuthStore {
    pub fn new(pg_pool: PgPool) -> Self {
        Self { pg_pool }
    }

    pub async fn register(
        &self,
        request: UserRegisterRequest,
    ) -> Result<UserSessionResponse, ApiError> {
        let username = normalize_username(&request.username)?;
        validate_password(&request.password)?;

        let existing = sqlx::query_scalar::<_, i64>(
            "select count(*) from users where lower(username) = $1",
        )
        .bind(username.as_str())
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;
        if existing > 0 {
            return Err(ApiError::Conflict("username already taken".to_string()));
        }

        let user_uuid = uuid::Uuid::now_v7();
        let user_id = format!("usr:{user_uuid}");
        let password_hash = hash_password(&request.password)?;
        let created_at = Utc::now();

        let mut tx = self
            .pg_pool
            .begin()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        sqlx::query(
            "insert into users (id, external_id, username, role, enabled, created_at) values ($1, $2, $3, $4, true, $5)",
        )
        .bind(user_uuid)
        .bind(user_id.as_str())
        .bind(username.as_str())
        .bind(USER_ROLE_DEVELOPER)
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
            create_session_tx(
                &mut tx,
                user_uuid,
                &user_id,
                &username,
                USER_ROLE_DEVELOPER,
                "fvs",
                created_at,
            )
            .await?;

        tx.commit()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(session)
    }

    pub async fn login(&self, request: UserLoginRequest) -> Result<UserSessionResponse, ApiError> {
        let username = normalize_username(&request.username)?;
        let record = sqlx::query_as::<_, UserPasswordRow>(
            "select u.id, u.external_id, u.username, u.role, u.enabled, pc.password_hash, pc.password_scheme
             from users u
             join password_credentials pc on pc.user_id = u.id
             where lower(u.username) = $1",
        )
        .bind(username.as_str())
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .ok_or_else(|| ApiError::Unauthorized("invalid username or password".to_string()))?;

        if !record.enabled {
            return Err(ApiError::Unauthorized("account is disabled".to_string()));
        }
        if record.password_scheme != PASSWORD_SCHEME_ARGON2ID {
            return Err(ApiError::Conflict(
                "user password credential must be migrated before login".to_string(),
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

        let session = create_session_tx(
            &mut tx,
            record.id,
            &record.external_id,
            &record.username,
            &record.role,
            "fvs",
            now,
        )
        .await?;

        tx.commit()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(session)
    }

    pub async fn authorize(&self, auth_header: Option<&str>) -> Result<UserIdentity, ApiError> {
        let token = bearer_token(auth_header)?;
        let session = authorize_session(&self.pg_pool, token).await?;
        Ok(UserIdentity {
            user_id: session.external_id,
            username: session.username,
            role: session.role,
        })
    }

    pub async fn authorize_admin(
        &self,
        auth_header: Option<&str>,
    ) -> Result<UserIdentity, ApiError> {
        let identity = self.authorize(auth_header).await?;
        if identity.role != USER_ROLE_ADMIN {
            return Err(ApiError::Unauthorized(
                "admin access is required".to_string(),
            ));
        }
        Ok(identity)
    }

    pub async fn current_session(
        &self,
        auth_header: Option<&str>,
    ) -> Result<UserSessionResponse, ApiError> {
        let token = bearer_token(auth_header)?;
        let session = authorize_session(&self.pg_pool, token).await?;
        Ok(UserSessionResponse {
            user_id: session.external_id,
            username: session.username,
            role: session.role,
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

    pub async fn list_users(&self) -> Result<Vec<UserAccount>, ApiError> {
        let rows = sqlx::query_as::<_, UserAccountRow>(
            "select external_id, username, role, created_at, enabled from users order by created_at asc",
        )
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(rows
            .into_iter()
            .map(|row| UserAccount {
                user_id: row.external_id,
                username: row.username,
                role: row.role,
                created_at: row.created_at,
                enabled: row.enabled,
            })
            .collect())
    }

    pub async fn create_user(&self, request: CreateUserRequest) -> Result<UserAccount, ApiError> {
        let username = normalize_username(&request.username)?;
        let role = normalize_user_role(&request.role)?;
        validate_password(&request.password)?;

        let existing = sqlx::query_scalar::<_, i64>(
            "select count(*) from users where lower(username) = $1",
        )
                .bind(username.as_str())
                .fetch_one(&self.pg_pool)
                .await
                .map_err(|error| ApiError::Internal(error.into()))?;
        if existing > 0 {
            return Err(ApiError::Conflict("username already taken".to_string()));
        }

        let user_uuid = uuid::Uuid::now_v7();
        let user_id = format!("usr:{user_uuid}");
        let created_at = Utc::now();
        let password_hash = hash_password(&request.password)?;

        let mut tx = self
            .pg_pool
            .begin()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        sqlx::query(
            "insert into users (id, external_id, username, role, enabled, created_at) values ($1, $2, $3, $4, true, $5)",
        )
        .bind(user_uuid)
        .bind(user_id.as_str())
        .bind(username.as_str())
        .bind(role)
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

        tx.commit()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(UserAccount {
            user_id,
            username,
            role: role.to_string(),
            created_at,
            enabled: true,
        })
    }

    pub async fn update_user_profile(
        &self,
        user_id: &str,
        request: &UpdateUserRequest,
    ) -> Result<(), ApiError> {
        let username = match request.username.as_deref() {
            Some(username) => Some(normalize_username(username)?),
            None => None,
        };
        let role = match request.role.as_deref() {
            Some(role) => Some(normalize_user_role(role)?),
            None => None,
        };
        if let Some(username) = username.as_deref() {
            let existing = sqlx::query_scalar::<_, i64>(
                "select count(*) from users where lower(username) = $1 and external_id <> $2",
            )
            .bind(username)
            .bind(user_id)
            .fetch_one(&self.pg_pool)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;
            if existing > 0 {
                return Err(ApiError::Conflict("username already taken".to_string()));
            }
        }

        let updated = sqlx::query(
            "update users
             set username = coalesce($2, username),
                 role = coalesce($3, role)
             where external_id = $1",
        )
        .bind(user_id)
        .bind(username.as_deref())
        .bind(role)
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .rows_affected();
        if updated == 0 {
            return Err(ApiError::NotFound("user not found".to_string()));
        }

        Ok(())
    }

    pub async fn set_enabled(&self, user_id: &str, enabled: bool) -> Result<(), ApiError> {
        let mut tx = self
            .pg_pool
            .begin()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        let updated = sqlx::query("update users set enabled = $2 where external_id = $1")
        .bind(user_id)
        .bind(enabled)
        .execute(&mut *tx)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .rows_affected();
        if updated == 0 {
            return Err(ApiError::NotFound("user not found".to_string()));
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
        validate_password(password)?;

        let user_uuid = sqlx::query_scalar::<_, uuid::Uuid>(
            "select id from users where external_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .ok_or_else(|| ApiError::NotFound("user not found".to_string()))?;

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
            return Err(ApiError::NotFound("user not found".to_string()));
        }

        Ok(())
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<(), ApiError> {
        let deleted = sqlx::query("delete from users where external_id = $1")
            .bind(user_id)
            .execute(&self.pg_pool)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?
            .rows_affected();
        if deleted == 0 {
            return Err(ApiError::NotFound("user not found".to_string()));
        }
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct UserPasswordRow {
    id: uuid::Uuid,
    external_id: String,
    username: String,
    role: String,
    enabled: bool,
    password_hash: String,
    password_scheme: String,
}

#[derive(sqlx::FromRow)]
struct AuthorizedSessionRow {
    external_id: String,
    username: String,
    role: String,
}

#[derive(sqlx::FromRow)]
struct UserAccountRow {
    external_id: String,
    username: String,
    role: String,
    created_at: DateTime<Utc>,
    enabled: bool,
}

async fn create_session_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_uuid: uuid::Uuid,
    external_id: &str,
    username: &str,
    role: &str,
    prefix: &str,
    now: DateTime<Utc>,
) -> Result<UserSessionResponse, ApiError> {
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

    Ok(UserSessionResponse {
        user_id: external_id.to_string(),
        username: username.to_string(),
        role: role.to_string(),
        token,
    })
}

async fn authorize_session(
    pg_pool: &PgPool,
    token: &str,
) -> Result<AuthorizedSessionRow, ApiError> {
    let session = sqlx::query_as::<_, AuthorizedSessionRow>(
        "select u.external_id, u.username, u.role
         from sessions s
         join users u on u.id = s.user_id
         where s.token_hash = $1
           and s.revoked_at is null
           and u.enabled = true",
    )
    .bind(hash_token(token))
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
