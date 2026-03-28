use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

pub const CURRENT_SCHEMA_VERSION: i32 = 1;
pub const CURRENT_PARSER_VERSION: i32 = 1;
pub const CURRENT_INDEX_VERSION: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DiscoveryScope {
    #[serde(rename = "same_host")]
    SameHost,
    #[default]
    #[serde(rename = "same_domain")]
    SameDomain,
    #[serde(rename = "any")]
    Any,
}

impl DiscoveryScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SameHost => "same_host",
            Self::SameDomain => "same_domain",
            Self::Any => "any",
        }
    }

    pub fn from_db_value(value: &str) -> Self {
        match value {
            "same_host" => Self::SameHost,
            "any" => Self::Any,
            _ => Self::SameDomain,
        }
    }
}

const TRACKING_PARAMS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "fbclid",
    "gclid",
    "msclkid",
    "_ga",
    "mc_cid",
    "mc_eid",
    "ref",
    "source",
    "campaign_id",
    "ad_id",
];

pub fn normalize_url(input: &str) -> Option<String> {
    let mut url = Url::parse(input).ok()?;

    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }

    if (url.scheme() == "https" && url.port() == Some(443))
        || (url.scheme() == "http" && url.port() == Some(80))
    {
        let _ = url.set_port(None);
    }

    url.set_fragment(None);

    let params: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| !TRACKING_PARAMS.contains(&key.as_ref()))
        .collect();

    if params.is_empty() {
        url.set_query(None);
    } else {
        let mut sorted = params;
        sorted.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let query = sorted
            .into_iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("&");
        url.set_query(Some(&query));
    }

    if url.path() != "/" {
        let normalized = url.path().trim_end_matches('/').to_string();
        if normalized.is_empty() {
            url.set_path("/");
        } else {
            url.set_path(&normalized);
        }
    }

    Some(url.to_string())
}

pub fn display_url(input: &str) -> String {
    Url::parse(input)
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

pub fn extract_host(input: &str) -> Option<String> {
    Url::parse(input)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_lowercase()))
}

pub fn origin_key(input: &str) -> Option<String> {
    let mut url = Url::parse(input).ok()?;

    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }

    if (url.scheme() == "https" && url.port() == Some(443))
        || (url.scheme() == "http" && url.port() == Some(80))
    {
        let _ = url.set_port(None);
    }

    let host = url.host_str()?.to_lowercase();
    let origin = match url.port() {
        Some(port) => format!("{}://{}:{}", url.scheme(), host, port),
        None => format!("{}://{}", url.scheme(), host),
    };

    Some(origin)
}

pub fn host_matches_scope(candidate_host: &str, anchor_host: &str, scope: DiscoveryScope) -> bool {
    match scope {
        DiscoveryScope::Any => true,
        DiscoveryScope::SameHost => candidate_host.eq_ignore_ascii_case(anchor_host),
        DiscoveryScope::SameDomain => {
            candidate_host.eq_ignore_ascii_case(anchor_host)
                || candidate_host
                    .to_lowercase()
                    .ends_with(&format!(".{}", anchor_host.to_lowercase()))
        }
    }
}

pub fn stable_document_id(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

pub fn content_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

pub fn derive_terms(title: &str, body: &str) -> Vec<String> {
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

pub fn word_count(input: &str) -> usize {
    input.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::{
        DiscoveryScope, content_hash, derive_terms, display_url, extract_host, host_matches_scope,
        normalize_url, origin_key, stable_document_id, word_count,
    };

    #[test]
    fn normalize_url_removes_tracking_and_ports() {
        assert_eq!(
            normalize_url("https://example.com:443/page/?utm_source=test&id=1"),
            Some("https://example.com/page?id=1".to_string())
        );
    }

    #[test]
    fn display_url_strips_scheme() {
        assert_eq!(display_url("https://example.com/a/b/"), "example.com/a/b");
    }

    #[test]
    fn extract_host_returns_lowercase_host() {
        assert_eq!(
            extract_host("https://Docs.Example.com/path"),
            Some("docs.example.com".to_string())
        );
    }

    #[test]
    fn origin_key_keeps_non_default_ports() {
        assert_eq!(
            origin_key("https://docs.example.com:8443/path?q=1"),
            Some("https://docs.example.com:8443".to_string())
        );
        assert_eq!(
            origin_key("https://docs.example.com:443/path"),
            Some("https://docs.example.com".to_string())
        );
    }

    #[test]
    fn stable_document_id_is_stable() {
        assert_eq!(
            stable_document_id("https://example.com"),
            stable_document_id("https://example.com")
        );
    }

    #[test]
    fn content_hash_changes_with_input() {
        assert_ne!(content_hash("a"), content_hash("b"));
    }

    #[test]
    fn content_hash_matches_sha256_hex() {
        assert_eq!(
            content_hash("hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn derive_terms_limits_output() {
        assert!(
            derive_terms("FindVerse Search Pipeline", "Search pipeline normalization").len() <= 12
        );
    }

    #[test]
    fn word_count_counts_tokens() {
        assert_eq!(word_count("one two   three"), 3);
    }

    #[test]
    fn same_domain_scope_allows_subdomains() {
        assert!(host_matches_scope(
            "docs.example.com",
            "example.com",
            DiscoveryScope::SameDomain
        ));
        assert!(!host_matches_scope(
            "example.net",
            "example.com",
            DiscoveryScope::SameDomain
        ));
    }
}
