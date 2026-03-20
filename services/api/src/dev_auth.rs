use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Context;
use chrono::{DateTime, Utc};
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{fs, sync::RwLock};
use uuid::Uuid;

use crate::{
    error::ApiError,
    models::{DevLoginRequest, DevRegisterRequest, DevSessionResponse},
};

#[derive(Debug, Clone)]
pub struct DevAuthStore {
    path: PathBuf,
    inner: Arc<RwLock<DevAuthState>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DevAuthState {
    #[serde(default)]
    accounts: HashMap<String, DevAccount>,
    #[serde(default)]
    sessions: HashMap<String, DevSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevAccount {
    pub user_id: String,
    pub username: String,
    password_hash: String,
    salt: String,
    pub created_at: DateTime<Utc>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DevSession {
    user_id: String,
    username: String,
    created_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DevUserIdentity {
    pub user_id: String,
}

impl DevAuthStore {
    pub async fn load(path: PathBuf) -> anyhow::Result<Self> {
        let empty = serde_json::to_string_pretty(&DevAuthState::default())?;
        if fs::metadata(&path).await.is_err() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(&path, &empty).await?;
        }

        let raw = fs::read_to_string(&path)
            .await
            .context("failed to read dev auth store")?;
        let state: DevAuthState =
            serde_json::from_str(&raw).context("failed to parse dev auth store")?;

        Ok(Self {
            path,
            inner: Arc::new(RwLock::new(state)),
        })
    }

    pub async fn register(&self, request: DevRegisterRequest) -> Result<DevSessionResponse, ApiError> {
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

        let mut state = self.inner.write().await;
        if state.accounts.values().any(|a| a.username == username) {
            return Err(ApiError::Conflict("username already taken".to_string()));
        }

        let salt = random_hex(16);
        let user_id = format!("dev:{}", Uuid::now_v7());
        let account = DevAccount {
            user_id: user_id.clone(),
            username: username.clone(),
            password_hash: hash_password(&salt, &request.password),
            salt,
            created_at: Utc::now(),
            enabled: true,
        };
        state.accounts.insert(user_id.clone(), account);

        let token = generate_session_token();
        state.sessions.insert(
            token.clone(),
            DevSession {
                user_id: user_id.clone(),
                username: username.clone(),
                created_at: Utc::now(),
                last_seen_at: Utc::now(),
            },
        );
        self.persist_locked(&state).await?;

        Ok(DevSessionResponse { user_id, username, token })
    }

    pub async fn login(&self, request: DevLoginRequest) -> Result<DevSessionResponse, ApiError> {
        let username = request.username.trim().to_lowercase();
        let mut state = self.inner.write().await;

        let account = state
            .accounts
            .values()
            .find(|a| a.username == username)
            .cloned()
            .ok_or_else(|| ApiError::Unauthorized("invalid username or password".to_string()))?;

        if !account.enabled {
            return Err(ApiError::Unauthorized("account is disabled".to_string()));
        }
        if hash_password(&account.salt, &request.password) != account.password_hash {
            return Err(ApiError::Unauthorized("invalid username or password".to_string()));
        }

        let token = generate_session_token();
        state.sessions.insert(
            token.clone(),
            DevSession {
                user_id: account.user_id.clone(),
                username: account.username.clone(),
                created_at: Utc::now(),
                last_seen_at: Utc::now(),
            },
        );
        self.persist_locked(&state).await?;

        Ok(DevSessionResponse {
            user_id: account.user_id,
            username: account.username,
            token,
        })
    }

    pub async fn authorize(&self, auth_header: Option<&str>) -> Result<DevUserIdentity, ApiError> {
        let token = bearer_token(auth_header)?.to_string();
        let mut state = self.inner.write().await;

        let session = state
            .sessions
            .get(&token)
            .ok_or_else(|| ApiError::Unauthorized("invalid session".to_string()))?
            .clone();

        let enabled = state
            .accounts
            .get(&session.user_id)
            .map(|a| a.enabled)
            .unwrap_or(false);
        if !enabled {
            return Err(ApiError::Unauthorized("account is disabled".to_string()));
        }

        if let Some(s) = state.sessions.get_mut(&token) {
            s.last_seen_at = Utc::now();
        }

        Ok(DevUserIdentity {
            user_id: session.user_id,
        })
    }

    pub async fn current_session(
        &self,
        auth_header: Option<&str>,
    ) -> Result<DevSessionResponse, ApiError> {
        let token = bearer_token(auth_header)?.to_string();
        let mut state = self.inner.write().await;

        let session = state
            .sessions
            .get(&token)
            .ok_or_else(|| ApiError::Unauthorized("invalid session".to_string()))?
            .clone();

        let enabled = state
            .accounts
            .get(&session.user_id)
            .map(|a| a.enabled)
            .unwrap_or(false);
        if !enabled {
            return Err(ApiError::Unauthorized("account is disabled".to_string()));
        }

        if let Some(s) = state.sessions.get_mut(&token) {
            s.last_seen_at = Utc::now();
        }

        Ok(DevSessionResponse {
            user_id: session.user_id,
            username: session.username,
            token,
        })
    }

    pub async fn logout(&self, auth_header: Option<&str>) -> Result<(), ApiError> {
        let token = bearer_token(auth_header)?.to_string();
        let mut state = self.inner.write().await;
        if state.sessions.remove(&token).is_none() {
            return Err(ApiError::Unauthorized("invalid session".to_string()));
        }
        self.persist_locked(&state).await?;
        Ok(())
    }

    pub async fn list_accounts(&self) -> Vec<DevAccount> {
        let state = self.inner.read().await;
        let mut accounts: Vec<DevAccount> = state.accounts.values().cloned().collect();
        accounts.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        accounts
    }

    pub async fn set_enabled(&self, user_id: &str, enabled: bool) -> Result<(), ApiError> {
        let mut state = self.inner.write().await;
        let account = state
            .accounts
            .get_mut(user_id)
            .ok_or_else(|| ApiError::NotFound("developer not found".to_string()))?;
        account.enabled = enabled;
        self.persist_locked(&state).await?;
        Ok(())
    }

    async fn persist_locked(&self, state: &DevAuthState) -> Result<(), ApiError> {
        let raw = serde_json::to_string_pretty(state)
            .map_err(|e| ApiError::Internal(e.into()))?;
        fs::write(&self.path, raw).await?;
        Ok(())
    }
}

fn bearer_token(auth_header: Option<&str>) -> Result<&str, ApiError> {
    let header =
        auth_header.ok_or_else(|| ApiError::Unauthorized("missing authorization".to_string()))?;
    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("invalid authorization scheme".to_string()))?
        .trim();
    if token.is_empty() {
        return Err(ApiError::Unauthorized("empty token".to_string()));
    }
    Ok(token)
}

fn hash_password(salt: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{salt}:{password}").as_bytes());
    format!("{:x}", hasher.finalize())
}

fn random_hex(chars: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(chars)
        .map(char::from)
        .collect::<String>()
        .to_lowercase()
}

fn generate_session_token() -> String {
    let secret = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect::<String>();
    format!("fvs_{secret}")
}

fn default_true() -> bool {
    true
}
