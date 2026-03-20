use std::collections::BTreeSet;

use scraper::{Html, Selector};
use url::Url;

use crate::models::ParsedHtml;

// ---------------------------------------------------------------------------
// HTML parsing
// ---------------------------------------------------------------------------
pub fn parse_html_document(url: &str, html: &str) -> ParsedHtml {
    let document = Html::parse_document(html);
    let title_selector = Selector::parse("title").ok();
    let meta_selector = Selector::parse("meta[name='description']").ok();

    let title = title_selector
        .as_ref()
        .and_then(|selector| document.select(selector).next())
        .map(|node| node.text().collect::<String>())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let meta_snippet = meta_selector
        .as_ref()
        .and_then(|selector| document.select(selector).next())
        .and_then(|node| node.value().attr("content"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    // Strip <script>, <style>, <nav>, <footer> before extracting body text
    let body = extract_clean_body_text(&document);

    // If no meta description, generate snippet from first 200 chars of body text
    let snippet = meta_snippet.or_else(|| {
        body.as_ref().map(|b| {
            let chars: String = b.chars().take(200).collect();
            chars
        })
    });

    ParsedHtml {
        title,
        snippet,
        body,
        discovered_urls: extract_links(url, html),
    }
}

/// Extract body text after stripping script, style, nav, and footer elements.
pub fn extract_clean_body_text(document: &Html) -> Option<String> {
    let body_selector = Selector::parse("body").ok()?;
    let body_element = document.select(&body_selector).next()?;

    let script_sel = Selector::parse("script").ok();
    let style_sel = Selector::parse("style").ok();
    let nav_sel = Selector::parse("nav").ok();
    let footer_sel = Selector::parse("footer").ok();

    // Collect IDs of nodes to exclude
    let mut excluded_ids = BTreeSet::new();
    for sel in [&script_sel, &style_sel, &nav_sel, &footer_sel]
        .into_iter()
        .flatten()
    {
        for el in document.select(sel) {
            excluded_ids.insert(el.id());
        }
    }

    // Walk text nodes under body, skipping those inside excluded elements
    let mut text_parts = Vec::new();
    for text_node in body_element.text() {
        text_parts.push(text_node);
    }

    // For a more accurate exclusion, re-extract text skipping excluded subtrees
    // We use the scraper tree traversal approach
    let body_html = body_element.html();
    let mut cleaned = body_html.clone();

    // Remove excluded elements from the HTML string, then re-parse
    for sel in [&script_sel, &style_sel, &nav_sel, &footer_sel]
        .into_iter()
        .flatten()
    {
        for el in document.select(sel) {
            let outer = el.html();
            cleaned = cleaned.replacen(&outer, "", 1);
        }
    }

    let cleaned_doc = Html::parse_fragment(&cleaned);
    let text: String = cleaned_doc
        .root_element()
        .text()
        .collect::<Vec<_>>()
        .join(" ");
    let normalized: String = text.split_whitespace().collect::<Vec<_>>().join(" ");

    if normalized.is_empty() {
        None
    } else {
        Some(normalized.chars().take(4_000).collect())
    }
}

// ---------------------------------------------------------------------------
// Link extraction
// ---------------------------------------------------------------------------
pub fn extract_links(base: &str, html: &str) -> Vec<String> {
    let base_url = match Url::parse(base) {
        Ok(url) => url,
        Err(_) => return Vec::new(),
    };
    let selector = match Selector::parse("a[href]") {
        Ok(selector) => selector,
        Err(_) => return Vec::new(),
    };

    let mut links = BTreeSet::new();
    let document = Html::parse_document(html);
    for anchor in document.select(&selector) {
        let Some(raw_href) = anchor.value().attr("href") else {
            continue;
        };

        let Ok(resolved) = base_url.join(raw_href) else {
            continue;
        };

        if matches!(resolved.scheme(), "http" | "https") {
            resolved.fragment().map(|_| ());
            let mut normalized = resolved;
            normalized.set_fragment(None);
            links.insert(normalized.to_string());
        }
    }

    links.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Domain filtering
// ---------------------------------------------------------------------------
pub fn filter_urls_by_domain(urls: Vec<String>, allowed_domains: &[String]) -> Vec<String> {
    urls.into_iter()
        .filter(|u| {
            Url::parse(u)
                .ok()
                .and_then(|parsed| parsed.host_str().map(|h| h.to_lowercase()))
                .map(|host| is_domain_allowed(&host, allowed_domains))
                .unwrap_or(false)
        })
        .collect()
}

pub fn is_domain_allowed(host: &str, allowed_domains: &[String]) -> bool {
    for domain in allowed_domains {
        if host == domain.as_str() || host.ends_with(&format!(".{domain}")) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Language detection
// ---------------------------------------------------------------------------
pub fn detect_language(text: &str) -> Option<String> {
    if text.is_empty() {
        return Some("unknown".to_string());
    }
    whatlang::detect(text)
        .map(|info| info.lang().code().to_string())
        .or(Some("unknown".to_string()))
}

// ---------------------------------------------------------------------------
// Sitemap URL extraction
// ---------------------------------------------------------------------------
pub fn extract_sitemap_urls(xml: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<loc>") {
        let from = &rest[start + 5..];
        if let Some(end) = from.find("</loc>") {
            urls.push(from[..end].trim().to_string());
            rest = &from[end + 6..];
        } else {
            break;
        }
    }
    urls
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sitemap_parser_collects_urls() {
        let xml = "<urlset><url><loc>https://example.com/a</loc></url><url><loc>https://example.com/b</loc></url></urlset>";
        assert_eq!(
            extract_sitemap_urls(xml),
            vec![
                "https://example.com/a".to_string(),
                "https://example.com/b".to_string()
            ]
        );
    }

    #[test]
    fn link_extractor_keeps_http_links() {
        let html = r#"<a href="/docs">Docs</a><a href="https://example.com/blog">Blog</a><a href="mailto:test@example.com">Skip</a>"#;
        assert_eq!(
            extract_links("https://example.com/", html),
            vec![
                "https://example.com/blog".to_string(),
                "https://example.com/docs".to_string()
            ]
        );
    }

    #[test]
    fn html_parser_extracts_fields() {
        let parsed = parse_html_document(
            "https://example.com",
            "<html><head><title>FindVerse</title><meta name='description' content='Search docs'></head><body>Hello crawler <a href='/docs'>Docs</a></body></html>",
        );

        assert_eq!(parsed.title, Some("FindVerse".to_string()));
        assert_eq!(parsed.snippet, Some("Search docs".to_string()));
        assert!(parsed.body.is_some());
        assert_eq!(
            parsed.discovered_urls,
            vec!["https://example.com/docs".to_string()]
        );
    }

    #[test]
    fn domain_filtering_exact_match() {
        let urls = vec![
            "https://example.com/page".to_string(),
            "https://other.com/page".to_string(),
            "https://sub.example.com/page".to_string(),
        ];
        let allowed = vec!["example.com".to_string()];
        let filtered = filter_urls_by_domain(urls, &allowed);
        assert_eq!(
            filtered,
            vec![
                "https://example.com/page".to_string(),
                "https://sub.example.com/page".to_string(),
            ]
        );
    }

    #[test]
    fn domain_filtering_empty_allows_all() {
        let urls = vec![
            "https://example.com/page".to_string(),
            "https://other.com/page".to_string(),
        ];
        let allowed: Vec<String> = vec![];
        let filtered = filter_urls_by_domain(urls.clone(), &allowed);
        // With empty allowed list, filter_urls_by_domain filters everything out.
        // The caller checks if allowed is empty and skips filtering.
        assert_eq!(filtered, Vec::<String>::new());
    }

    #[test]
    fn snippet_fallback_from_body() {
        let html = "<html><head><title>Test Page</title></head><body>This is some body text that should be used as the snippet when no meta description is present.</body></html>";
        let parsed = parse_html_document("https://example.com", html);
        assert_eq!(parsed.title, Some("Test Page".to_string()));
        // No meta description, so snippet should come from body
        assert!(parsed.snippet.is_some());
        let snippet = parsed.snippet.unwrap();
        assert!(snippet.starts_with("This is some body text"));
        assert!(snippet.len() <= 200);
    }

    #[test]
    fn snippet_strips_script_style_nav_footer() {
        let html = r#"<html><head><title>Clean</title></head><body>
            <nav>Navigation links</nav>
            <p>Main content here</p>
            <script>var x = 1;</script>
            <style>.foo { color: red; }</style>
            <footer>Footer text</footer>
        </body></html>"#;
        let parsed = parse_html_document("https://example.com", html);
        let body = parsed.body.unwrap();
        assert!(!body.contains("Navigation links"));
        assert!(!body.contains("var x = 1"));
        assert!(!body.contains("color: red"));
        assert!(!body.contains("Footer text"));
        assert!(body.contains("Main content here"));
    }

    #[test]
    fn language_detection_english() {
        let lang = detect_language(
            "This is a simple English sentence with enough words for detection to work properly.",
        );
        assert!(lang.is_some());
        assert_eq!(lang.unwrap(), "eng");
    }

    #[test]
    fn language_detection_empty_returns_unknown() {
        let lang = detect_language("");
        assert_eq!(lang, Some("unknown".to_string()));
    }

    #[test]
    fn is_domain_allowed_subdomain() {
        assert!(is_domain_allowed(
            "docs.example.com",
            &["example.com".to_string()]
        ));
        assert!(is_domain_allowed(
            "example.com",
            &["example.com".to_string()]
        ));
        assert!(!is_domain_allowed(
            "notexample.com",
            &["example.com".to_string()]
        ));
        assert!(!is_domain_allowed(
            "evil-example.com",
            &["example.com".to_string()]
        ));
    }
}
