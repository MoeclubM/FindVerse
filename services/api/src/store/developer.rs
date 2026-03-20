use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Context;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tokio::{fs, sync::RwLock as AsyncRwLock};
use uuid::Uuid;

use crate::{
    error::ApiError,
    models::{
        AdminDeveloperRecord, ApiKeyMetadata, CreateKeyRequest, CreatedKeyResponse,
        DeveloperUsageResponse, UpdateDeveloperRequest,
    },
};

use super::{atomic_write, bearer_hash, ensure_file_with_fallbacks, generate_token, hash_token};

#[derive(Debug, Clone)]
pub struct DeveloperStore {
    path: PathBuf,
    inner: Arc<AsyncRwLock<DeveloperStoreState>>,
    qps_tracker: Arc<Mutex<HashMap<String, VecDeque<Instant>>>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DeveloperStoreState {
    #[serde(default)]
    records: HashMap<String, DeveloperRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeveloperRecord {
    developer_id: String,
    #[serde(default = "default_qps_limit")]
    qps_limit: u32,
    #[serde(default = "default_daily_limit")]
    daily_limit: u32,
    #[serde(default)]
    used_today: u32,
    #[serde(default = "default_usage_day")]
    usage_day: NaiveDate,
    #[serde(default)]
    keys: Vec<StoredKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredKey {
    id: String,
    name: String,
    preview: String,
    token_hash: String,
    created_at: DateTime<Utc>,
    #[serde(default)]
    revoked_at: Option<DateTime<Utc>>,
}

impl DeveloperStore {
    pub async fn load(path: PathBuf) -> anyhow::Result<Self> {
        let empty = serde_json::to_string_pretty(&DeveloperStoreState::default())?;
        ensure_file_with_fallbacks(
            &path,
            &empty,
            &[
                PathBuf::from("/opt/findverse/developer_store.json"),
                PathBuf::from("services/api/fixtures/developer_store.json"),
            ],
        )
        .await?;

        let raw = fs::read_to_string(&path)
            .await
            .context("failed to read developer store file")?;
        let state: DeveloperStoreState =
            serde_json::from_str(&raw).context("failed to parse developer store file")?;

        Ok(Self {
            path,
            inner: Arc::new(AsyncRwLock::new(state)),
            qps_tracker: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn create_key(
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

        let token = generate_token("fvk");
        let token_hash = hash_token(&token);
        let preview = format!("{}...{}", &token[..8], &token[token.len() - 4..]);
        let created_at = Utc::now();
        let key = StoredKey {
            id: Uuid::now_v7().to_string(),
            name: clean_name.to_string(),
            preview: preview.clone(),
            token_hash,
            created_at,
            revoked_at: None,
        };

        {
            let mut state = self.inner.write().await;
            let record = state
                .records
                .entry(developer_id.to_string())
                .or_insert_with(|| DeveloperRecord {
                    developer_id: developer_id.to_string(),
                    qps_limit: 5,
                    daily_limit: 10_000,
                    used_today: 0,
                    usage_day: Utc::now().date_naive(),
                    keys: Vec::new(),
                });

            record.keys.push(key.clone());
            self.persist_locked(&state).await?;
        }

        Ok(CreatedKeyResponse {
            id: key.id,
            name: key.name,
            preview,
            token,
            created_at,
        })
    }

    pub async fn revoke_key(&self, developer_id: &str, key_id: &str) -> Result<(), ApiError> {
        let mut state = self.inner.write().await;
        let record = state
            .records
            .get_mut(developer_id)
            .ok_or_else(|| ApiError::NotFound("developer record not found".to_string()))?;

        let key = record
            .keys
            .iter_mut()
            .find(|key| key.id == key_id)
            .ok_or_else(|| ApiError::NotFound("api key not found".to_string()))?;

        if key.revoked_at.is_some() {
            return Err(ApiError::Conflict("api key already revoked".to_string()));
        }

        key.revoked_at = Some(Utc::now());
        self.persist_locked(&state).await?;
        Ok(())
    }

    pub async fn usage(&self, developer_id: &str) -> Result<DeveloperUsageResponse, ApiError> {
        let mut state = self.inner.write().await;
        let record = state
            .records
            .entry(developer_id.to_string())
            .or_insert_with(|| DeveloperRecord {
                developer_id: developer_id.to_string(),
                qps_limit: 5,
                daily_limit: 10_000,
                used_today: 0,
                usage_day: Utc::now().date_naive(),
                keys: Vec::new(),
            });

        roll_usage_day(record);
        let response = DeveloperUsageResponse {
            developer_id: record.developer_id.clone(),
            qps_limit: record.qps_limit,
            daily_limit: record.daily_limit,
            used_today: record.used_today,
            keys: record
                .keys
                .iter()
                .map(|key| ApiKeyMetadata {
                    id: key.id.clone(),
                    name: key.name.clone(),
                    preview: key.preview.clone(),
                    created_at: key.created_at,
                    revoked_at: key.revoked_at,
                })
                .collect(),
        };

        self.persist_locked(&state).await?;
        Ok(response)
    }

    pub async fn validate_and_track(&self, auth_header: Option<&str>) -> Result<(), ApiError> {
        let Some(header) = auth_header else {
            return Err(ApiError::Unauthorized("api key required".to_string()));
        };

        let token_hash = bearer_hash(header)?;
        let today = Utc::now().date_naive();
        let now = Instant::now();
        let mut state = self.inner.write().await;

        for record in state.records.values_mut() {
            roll_usage_day(record);

            if !record
                .keys
                .iter()
                .any(|key| key.token_hash == token_hash && key.revoked_at.is_none())
            {
                continue;
            }

            // QPS sliding-window check
            if record.qps_limit > 0 {
                let mut tracker = self
                    .qps_tracker
                    .lock()
                    .map_err(|_| ApiError::Internal(anyhow::anyhow!("qps tracker poisoned")))?;
                let window = tracker.entry(record.developer_id.clone()).or_default();
                let cutoff = now - Duration::from_secs(1);
                window.retain(|ts| *ts > cutoff);
                if window.len() >= record.qps_limit as usize {
                    return Err(ApiError::TooManyRequests("rate limit exceeded".to_string()));
                }
                window.push_back(now);
            }

            // Daily quota check
            if record.used_today >= record.daily_limit {
                return Err(ApiError::TooManyRequests(
                    "daily request quota exceeded".to_string(),
                ));
            }

            record.used_today += 1;
            record.usage_day = today;
            self.persist_locked(&state).await?;
            return Ok(());
        }

        Err(ApiError::Unauthorized("invalid api key".to_string()))
    }

    /// Returns developer_id for the given api key without tracking usage.
    /// Used by the crawler hello endpoint to identify who owns the key.
    pub async fn validate_api_key_for_identity(
        &self,
        auth_header: Option<&str>,
    ) -> Result<String, ApiError> {
        let Some(header) = auth_header else {
            return Err(ApiError::Unauthorized("api key required".to_string()));
        };
        let token_hash = bearer_hash(header)?;
        let state = self.inner.read().await;
        for record in state.records.values() {
            if record
                .keys
                .iter()
                .any(|k| k.token_hash == token_hash && k.revoked_at.is_none())
            {
                return Ok(record.developer_id.clone());
            }
        }
        Err(ApiError::Unauthorized("invalid api key".to_string()))
    }

    /// Returns all developer records for the admin developers list endpoint.
    pub async fn list_all_usage(&self) -> Vec<DeveloperUsageResponse> {
        let mut state = self.inner.write().await;
        let mut result = Vec::new();
        for record in state.records.values_mut() {
            roll_usage_day(record);
            result.push(DeveloperUsageResponse {
                developer_id: record.developer_id.clone(),
                qps_limit: record.qps_limit,
                daily_limit: record.daily_limit,
                used_today: record.used_today,
                keys: record
                    .keys
                    .iter()
                    .map(|k| ApiKeyMetadata {
                        id: k.id.clone(),
                        name: k.name.clone(),
                        preview: k.preview.clone(),
                        created_at: k.created_at,
                        revoked_at: k.revoked_at,
                    })
                    .collect(),
            });
        }
        result
    }

    /// Admin: update QPS/daily quota for a developer.
    pub async fn update_quota(
        &self,
        developer_id: &str,
        request: UpdateDeveloperRequest,
    ) -> Result<(), ApiError> {
        let mut state = self.inner.write().await;
        let record = state
            .records
            .get_mut(developer_id)
            .ok_or_else(|| ApiError::NotFound("developer not found".to_string()))?;
        if let Some(qps) = request.qps_limit {
            record.qps_limit = qps.max(1);
        }
        if let Some(daily) = request.daily_limit {
            record.daily_limit = daily.max(1);
        }
        self.persist_locked(&state).await?;
        Ok(())
    }

    /// Merge dev auth account info with quota data to build admin view.
    pub fn build_admin_developer_record(
        usage: &DeveloperUsageResponse,
        username: &str,
        enabled: bool,
        created_at: chrono::DateTime<Utc>,
    ) -> AdminDeveloperRecord {
        AdminDeveloperRecord {
            user_id: usage.developer_id.clone(),
            username: username.to_string(),
            enabled,
            created_at,
            qps_limit: usage.qps_limit,
            daily_limit: usage.daily_limit,
            used_today: usage.used_today,
            key_count: usage.keys.len(),
        }
    }

    async fn persist_locked(&self, state: &DeveloperStoreState) -> Result<(), ApiError> {
        let raw =
            serde_json::to_string_pretty(state).map_err(|error| ApiError::Internal(error.into()))?;
        atomic_write(&self.path, &raw).await?;
        Ok(())
    }
}

fn roll_usage_day(record: &mut DeveloperRecord) {
    let today = Utc::now().date_naive();
    if record.usage_day != today {
        record.usage_day = today;
        record.used_today = 0;
    }
}

fn default_qps_limit() -> u32 {
    5
}

fn default_daily_limit() -> u32 {
    10_000
}

fn default_usage_day() -> NaiveDate {
    Utc::now().date_naive()
}
