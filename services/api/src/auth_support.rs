use anyhow::anyhow;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use sha2::{Digest, Sha256};

use crate::error::ApiError;

pub(crate) const PASSWORD_SCHEME_ARGON2ID: &str = "argon2id";
pub(crate) const PASSWORD_SCHEME_LEGACY_SHA256_SALT_V1: &str = "legacy-sha256-salt-v1";

pub(crate) fn bearer_token(auth_header: Option<&str>) -> Result<&str, ApiError> {
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

pub(crate) fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| ApiError::Internal(anyhow!(error.to_string())))
}

pub(crate) fn verify_password(
    password: &str,
    password_hash: &str,
    password_scheme: &str,
    password_salt: Option<&str>,
) -> Result<bool, ApiError> {
    match password_scheme {
        PASSWORD_SCHEME_ARGON2ID => {
            let parsed_hash = PasswordHash::new(password_hash)
                .map_err(|error| ApiError::Internal(anyhow!(error.to_string())))?;
            Ok(Argon2::default()
                .verify_password(password.as_bytes(), &parsed_hash)
                .is_ok())
        }
        PASSWORD_SCHEME_LEGACY_SHA256_SALT_V1 => {
            let salt = password_salt.ok_or_else(|| {
                ApiError::Internal(anyhow!("legacy password hash is missing required salt"))
            })?;
            Ok(legacy_hash_password(salt, password) == password_hash)
        }
        other => Err(ApiError::Internal(anyhow!(format!(
            "unsupported password scheme {other}"
        )))),
    }
}

pub(crate) fn legacy_hash_password(salt: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{salt}:{password}").as_bytes());
    format!("{:x}", hasher.finalize())
}
