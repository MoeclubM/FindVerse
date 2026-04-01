use anyhow::anyhow;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};

use crate::error::ApiError;

pub(crate) const PASSWORD_SCHEME_ARGON2ID: &str = "argon2id";

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

pub(crate) fn verify_password(password: &str, password_hash: &str) -> Result<bool, ApiError> {
    let parsed_hash = PasswordHash::new(password_hash)
        .map_err(|error| ApiError::Internal(anyhow!(error.to_string())))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::{hash_password, verify_password};

    #[test]
    fn verify_password_accepts_argon2id_hash() {
        let password = "secret";
        let hash = hash_password(password).expect("argon2 hash should be generated");

        let verified = verify_password(password, &hash).expect("argon2 verification should work");

        assert!(verified);
    }
}
