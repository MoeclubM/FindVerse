pub mod developer;
pub mod search;

pub use developer::DeveloperStore;
pub use search::SearchIndex;

use std::path::PathBuf;

use sha2::{Digest, Sha256};
use tokio::fs;

use crate::error::ApiError;

pub fn tokenize(input: &str) -> Vec<String> {
    let mut token = String::new();
    let mut tokens = Vec::new();

    for ch in input.chars() {
        if ch.is_alphanumeric() {
            token.extend(ch.to_lowercase());
        } else if !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
        }
    }

    if !token.is_empty() {
        tokens.push(token);
    }

    tokens
}

pub fn derive_terms(title: &str, body: &str) -> Vec<String> {
    use std::collections::BTreeSet;

    let mut terms = BTreeSet::new();
    for source in [title, body] {
        for token in source
            .split(|ch: char| !ch.is_alphanumeric())
            .map(str::trim)
            .filter(|token| token.len() >= 4)
        {
            terms.insert(token.to_lowercase());
            if terms.len() >= 12 {
                return terms.into_iter().collect();
            }
        }
    }
    terms.into_iter().collect()
}

pub fn display_url(input: &str) -> String {
    url::Url::parse(input)
        .ok()
        .and_then(|url| {
            let host = url.host_str()?.to_string();
            let path = url.path().trim_end_matches('/').to_string();
            Some(if path.is_empty() {
                host
            } else {
                format!("{host}{path}")
            })
        })
        .unwrap_or_else(|| input.to_string())
}

pub fn stable_document_id(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    format!("{:x}", hasher.finalize())
}

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
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) fn generate_token(prefix: &str) -> String {
    use rand::{Rng, distr::Alphanumeric};

    let secret = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect::<String>();
    format!("{prefix}_{secret}")
}

pub(crate) async fn atomic_write(path: &PathBuf, contents: &str) -> Result<(), std::io::Error> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents).await?;
    fs::rename(&tmp, path).await?;
    Ok(())
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
