use anyhow::anyhow;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};

use crate::error::ApiError;

pub(crate) const PASSWORD_SCHEME_ARGON2ID: &str = "argon2id";
pub(crate) const USER_ROLE_ADMIN: &str = "admin";
pub(crate) const USER_ROLE_DEVELOPER: &str = "developer";

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

pub(crate) fn normalize_username(username: &str) -> Result<String, ApiError> {
    let username = username.trim().to_lowercase();
    if username.len() < 3
        || username
            .chars()
            .any(|c| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
    {
        return Err(ApiError::BadRequest(
            "username must be 3+ alphanumeric characters (_, - allowed)".to_string(),
        ));
    }

    Ok(username)
}

pub(crate) fn validate_password(password: &str) -> Result<(), ApiError> {
    if password.len() < 8 {
        return Err(ApiError::BadRequest(
            "password must be at least 8 characters".to_string(),
        ));
    }

    Ok(())
}

pub(crate) fn normalize_user_role(role: &str) -> Result<&'static str, ApiError> {
    match role.trim() {
        USER_ROLE_ADMIN => Ok(USER_ROLE_ADMIN),
        USER_ROLE_DEVELOPER => Ok(USER_ROLE_DEVELOPER),
        _ => Err(ApiError::BadRequest(
            "role must be admin or developer".to_string(),
        )),
    }
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
