pub mod developer;
pub mod search;

pub use developer::DeveloperStore;
pub use search::SearchIndex;

use std::path::PathBuf;

pub use findverse_common::{
    CURRENT_INDEX_VERSION, CURRENT_PARSER_VERSION, CURRENT_SCHEMA_VERSION, content_hash,
    derive_terms, display_url, extract_host, normalize_url, stable_document_id, word_count,
};
use tokio::fs;

use crate::error::ApiError;

pub(crate) fn bearer_hash(header: &str) -> Result<String, ApiError> {
    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("invalid authorization scheme".to_string()))?
        .trim();

    if token.is_empty() {
        return Err(ApiError::Unauthorized("empty bearer token".to_string()));
    }

    Ok(hash_token(token))
}

pub(crate) fn hash_token(token: &str) -> String {
    content_hash(token)
}

pub(crate) fn generate_token(prefix: &str) -> String {
    use rand::{RngExt, distr::Alphanumeric};

    let secret = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect::<String>();
    format!("{prefix}_{secret}")
}

pub(crate) async fn ensure_file_with_fallbacks(
    path: &PathBuf,
    default_contents: &str,
    fallbacks: &[PathBuf],
) -> anyhow::Result<()> {
    if fs::metadata(path).await.is_ok() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    for fallback in fallbacks {
        if fallback != path && fs::metadata(fallback).await.is_ok() {
            fs::copy(fallback, path).await?;
            return Ok(());
        }
    }

    fs::write(path, default_contents).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{generate_token, hash_token};

    #[test]
    fn hash_token_is_stable_and_hex_encoded() {
        let hash = hash_token("hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert!(
            hash.chars()
                .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
        );
    }

    #[test]
    fn generate_token_keeps_prefix_and_expected_length() {
        let token = generate_token("fvc");

        assert!(token.starts_with("fvc_"));
        assert_eq!(token.len(), 44);
        assert!(
            token["fvc_".len()..]
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric())
        );
    }
}
