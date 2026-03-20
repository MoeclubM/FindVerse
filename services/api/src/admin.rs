use std::{
    collections::HashMap,
    sync::Arc,
};

use chrono::{DateTime, Utc};
use rand::{Rng, distr::Alphanumeric};
use tokio::sync::RwLock;

use crate::{
    error::ApiError,
    models::{AdminLoginRequest, AdminSessionResponse},
};

#[derive(Debug, Clone)]
pub struct AdminAuth {
    username: String,
    password: String,
    developer_id: String,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
}

#[derive(Debug, Clone)]
struct SessionRecord {
    username: String,
    developer_id: String,
    last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AdminIdentity {
    pub developer_id: String,
}

impl AdminAuth {
    pub fn new(username: String, password: String) -> Self {
        Self {
            developer_id: format!("local:{username}"),
            username,
            password,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn login(
        &self,
        request: AdminLoginRequest,
    ) -> Result<AdminSessionResponse, ApiError> {
        if request.username.trim() != self.username || request.password != self.password {
            return Err(ApiError::Unauthorized("invalid username or password".to_string()));
        }

        let token = generate_token();
        let now = Utc::now();
        self.sessions.write().await.insert(
            token.clone(),
            SessionRecord {
                username: self.username.clone(),
                developer_id: self.developer_id.clone(),
                last_seen_at: now,
            },
        );

        Ok(AdminSessionResponse {
            developer_id: self.developer_id.clone(),
            username: self.username.clone(),
            token,
        })
    }

    pub async fn authorize(&self, auth_header: Option<&str>) -> Result<AdminIdentity, ApiError> {
        let token = bearer_token(auth_header)?;
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(token)
            .ok_or_else(|| ApiError::Unauthorized("admin session is invalid".to_string()))?;
        session.last_seen_at = Utc::now();

        Ok(AdminIdentity {
            developer_id: session.developer_id.clone(),
        })
    }

    pub async fn current_session(
        &self,
        auth_header: Option<&str>,
    ) -> Result<AdminSessionResponse, ApiError> {
        let token = bearer_token(auth_header)?.to_string();
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(&token)
            .ok_or_else(|| ApiError::Unauthorized("admin session is invalid".to_string()))?;
        session.last_seen_at = Utc::now();

        Ok(AdminSessionResponse {
            developer_id: session.developer_id.clone(),
            username: session.username.clone(),
            token,
        })
    }

    pub async fn logout(&self, auth_header: Option<&str>) -> Result<(), ApiError> {
        let token = bearer_token(auth_header)?.to_string();
        let removed = self.sessions.write().await.remove(&token);
        if removed.is_none() {
            return Err(ApiError::Unauthorized("admin session is invalid".to_string()));
        }

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
        return Err(ApiError::Unauthorized("empty bearer token".to_string()));
    }

    Ok(token)
}

fn generate_token() -> String {
    let secret = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect::<String>();
    format!("fva_{secret}")
}
