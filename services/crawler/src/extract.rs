use std::collections::BTreeSet;

use findverse_common::normalize_url;
use scraper::{
    ElementRef, Html, Selector,
    node::{Element, Node},
};
use url::Url;

use crate::models::{ParsedHtml, RobotsDirectives};

// ---------------------------------------------------------------------------
// HTML parsing
// ---------------------------------------------------------------------------
pub fn parse_html_document(url: &str, html: &str) -> ParsedHtml {
    let document = Html::parse_document(html);
    let title_selector = Selector::parse("title").ok();
    let meta_selector = Selector::parse("meta[name='description']").ok();
    let og_title_selector = Selector::parse("meta[property='og:title']").ok();
    let og_desc_selector = Selector::parse("meta[property='og:description']").ok();
    let canonical_selector = Selector::parse("link[rel='canonical']").ok();
    let robots_directives = extract_meta_robots_directives(&document);

    let title = og_title_selector
        .as_ref()
        .and_then(|s| document.select(s).next())
        .and_then(|n| n.value().attr("content"))
        .and_then(sanitize_extracted_text)
        .or_else(|| {
            title_selector
                .as_ref()
                .and_then(|selector| document.select(selector).next())
                .map(|node| node.text().collect::<String>())
                .and_then(|value| sanitize_extracted_text(&value))
        });

    let meta_snippet = og_desc_selector
        .as_ref()
        .and_then(|s| document.select(s).next())
        .and_then(|n| n.value().attr("content"))
        .and_then(sanitize_extracted_text)
        .or_else(|| {
            meta_selector
                .as_ref()
                .and_then(|selector| document.select(selector).next())
                .and_then(|node| node.value().attr("content"))
                .and_then(sanitize_extracted_text)
        });

    let body = extract_clean_body_text(&document);

    let snippet = meta_snippet
        .filter(|value| !looks_like_low_signal_snippet(value))
        .or_else(|| {
            body.as_ref().map(|b| {
                let chars: String = b.chars().take(200).collect();
                chars
            })
        });

    let canonical_url = canonical_selector
        .as_ref()
        .and_then(|selector| document.select(selector).next())
        .and_then(|node| node.value().attr("href"))
        .and_then(|href| Url::parse(url).ok().and_then(|base| base.join(href).ok()))
        .and_then(|resolved| normalize_url(resolved.as_ref()));

    ParsedHtml {
        title,
        snippet,
        body,
        discovered_urls: extract_links(url, html),
        canonical_url,
        robots_directives,
    }
}

pub fn parse_x_robots_tag_values<'a>(
    values: impl IntoIterator<Item = &'a str>,
) -> RobotsDirectives {
    let mut directives = RobotsDirectives::default();

    for value in values {
        let mut scoped_agent: Option<String> = None;
        for raw_segment in value.split(',') {
            let segment = raw_segment.trim();
            if segment.is_empty() {
                continue;
            }

            let (agent, directive_value) = if let Some((agent, rest)) = segment.split_once(':') {
                let agent = agent.trim().to_ascii_lowercase();
                scoped_agent = Some(agent.clone());
                (Some(agent), rest.trim())
            } else {
                (scoped_agent.clone(), segment)
            };

            if applies_to_findverse(agent.as_deref()) {
                directives.merge(parse_robots_directives(directive_value));
            }
        }
    }

    directives
}

/// Extract body text after stripping script, style, nav, and footer elements.
pub fn extract_clean_body_text(document: &Html) -> Option<String> {
    let root = select_content_root(document)?;
    let mut parts = Vec::new();
    collect_visible_text(root, &mut parts);
    sanitize_extracted_text(&parts.join(" "))
}

// ---------------------------------------------------------------------------
// Link extraction
// ---------------------------------------------------------------------------
pub fn extract_links(base: &str, html: &str) -> Vec<String> {
    let base_url = match Url::parse(base) {
        Ok(url) => url,
        Err(_) => return Vec::new(),
    };

    let mut links = BTreeSet::new();
    let document = Html::parse_document(html);

    // 提取 <a href>
    if let Ok(selector) = Selector::parse("a[href]") {
        for anchor in document.select(&selector) {
            if let Some(href) = anchor.value().attr("href") {
                if let Ok(resolved) = base_url.join(href) {
                    if let Some(normalized) = normalize_url(resolved.as_ref()) {
                        links.insert(normalized);
                    }
                }
            }
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

fn extract_meta_robots_directives(document: &Html) -> RobotsDirectives {
    let mut directives = RobotsDirectives::default();
    let Ok(meta_selector) = Selector::parse("meta[name][content]") else {
        return directives;
    };

    for meta in document.select(&meta_selector) {
        let Some(name) = meta.value().attr("name") else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        if name != "robots" && name != "findverse" && name != "findversebot" {
            continue;
        }
        let Some(content) = meta.value().attr("content") else {
            continue;
        };
        directives.merge(parse_robots_directives(content));
    }

    directives
}

fn parse_robots_directives(value: &str) -> RobotsDirectives {
    let mut directives = RobotsDirectives::default();

    for token in value.split([',', ';']) {
        match token.trim().to_ascii_lowercase().as_str() {
            "none" => {
                directives.noindex = true;
                directives.nofollow = true;
            }
            "noindex" => directives.noindex = true,
            "nofollow" => directives.nofollow = true,
            _ => {}
        }
    }

    directives
}

fn applies_to_findverse(agent: Option<&str>) -> bool {
    matches!(
        agent,
        None | Some("*") | Some("findverse") | Some("findversebot")
    )
}

fn select_content_root<'a>(document: &'a Html) -> Option<ElementRef<'a>> {
    for selector in [
        "#mw-content-text",
        "article",
        "main",
        "[role='main']",
        ".mw-parser-output",
        ".article-content",
        ".entry-content",
        ".post-content",
        ".article-body",
        "#content",
        "#main-content",
        ".mw-body-content",
        "body",
    ] {
        let Ok(parsed) = Selector::parse(selector) else {
            continue;
        };
        if let Some(element) = document.select(&parsed).next() {
            return Some(element);
        }
    }
    None
}

fn collect_visible_text(root: ElementRef<'_>, parts: &mut Vec<String>) {
    for child in root.children() {
        collect_visible_text_node(child.value(), child.children(), parts);
    }
}

fn collect_visible_text_node<'a>(
    node: &Node,
    children: impl Iterator<Item = ego_tree::NodeRef<'a, Node>>,
    parts: &mut Vec<String>,
) {
    match node {
        Node::Text(text) => {
            let normalized = normalize_whitespace(text);
            if !normalized.is_empty() && !looks_like_noise_fragment(&normalized) {
                parts.push(normalized);
            }
        }
        Node::Element(element) => {
            if should_skip_element(element) {
                return;
            }
            for child in children {
                collect_visible_text_node(child.value(), child.children(), parts);
            }
            if is_block_element(element.name()) {
                parts.push("\n".to_string());
            }
        }
        _ => {
            for child in children {
                collect_visible_text_node(child.value(), child.children(), parts);
            }
        }
    }
}

fn should_skip_element(element: &Element) -> bool {
    let tag = element.name();
    if matches!(
        tag,
        "script"
            | "style"
            | "noscript"
            | "nav"
            | "footer"
            | "header"
            | "aside"
            | "form"
            | "button"
            | "svg"
            | "canvas"
            | "iframe"
    ) {
        return true;
    }

    if element.attr("hidden").is_some()
        || element
            .attr("aria-hidden")
            .is_some_and(|value| value.eq_ignore_ascii_case("true"))
    {
        return true;
    }

    if element.attr("style").is_some_and(looks_like_hidden_style) {
        return true;
    }

    if element
        .attr("id")
        .is_some_and(|value| has_noisy_token(value, &["toc", "catlinks", "footer", "sitenotice"]))
    {
        return true;
    }

    element.attr("class").is_some_and(|value| {
        has_noisy_token(
            value,
            &[
                "navbox",
                "navbar",
                "noprint",
                "nomobile",
                "infobox",
                "infobox-incompleted",
                "mw-editsection",
                "mw-empty-elt",
                "metadata",
                "catlinks",
                "toc",
                "colorededit",
                "plainlinks",
                "sistersitebox",
                "vertical-navbox",
                "navbox-title",
                "navbox-group",
                "navbox-list",
                "portalbox",
            ],
        )
    })
}

fn looks_like_hidden_style(style: &str) -> bool {
    let lower = style.to_ascii_lowercase().replace(' ', "");
    lower.contains("display:none") || lower.contains("visibility:hidden")
}

fn has_noisy_token(value: &str, tokens: &[&str]) -> bool {
    value
        .split(|ch: char| ch.is_ascii_whitespace() || ch == '-' || ch == '_' || ch == ':')
        .filter(|segment| !segment.is_empty())
        .any(|segment| {
            let lower = segment.to_ascii_lowercase();
            tokens.iter().any(|token| lower == *token)
        })
}

fn is_block_element(tag: &str) -> bool {
    matches!(
        tag,
        "article"
            | "aside"
            | "blockquote"
            | "br"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "figcaption"
            | "figure"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "hr"
            | "li"
            | "main"
            | "p"
            | "section"
            | "table"
            | "tbody"
            | "td"
            | "th"
            | "thead"
            | "tr"
            | "ul"
            | "ol"
    )
}

fn sanitize_extracted_text(value: &str) -> Option<String> {
    let mut current = normalize_whitespace(value);
    if current.is_empty() {
        return None;
    }

    for _ in 0..2 {
        let parsed = Html::parse_fragment(&current);
        let collapsed =
            normalize_whitespace(&parsed.root_element().text().collect::<Vec<_>>().join(" "));
        if collapsed.is_empty() {
            return None;
        }
        if collapsed == current {
            break;
        }
        current = collapsed;
    }

    Some(current)
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn looks_like_low_signal_snippet(value: &str) -> bool {
    value.contains("欢迎您参与完善")
        || value.contains("编辑前请阅读")
        || value.contains("协助编辑本条目")
        || value.contains(".mw-parser-output")
        || value.contains("Wikiplus")
        || looks_like_noise_fragment(value)
}

fn looks_like_noise_fragment(value: &str) -> bool {
    value.contains("RLSTATE=")
        || value.contains("RLPAGEMODULES=")
        || value.contains(".mw-parser-output")
        || value.contains("Wikiplus-Edit-EveryWhereBtn")
        || (value.contains('{') && value.contains('}') && value.contains(':'))
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
    fn parse_html_extracts_canonical_and_alternates_without_enqueueing_them() {
        let html = r#"
        <html>
          <head>
            <title>Docs</title>
            <link rel="canonical" href="/guide" />
            <link rel="alternate" hreflang="zh-CN" href="/zh/guide" />
          </head>
          <body><a href="/install">Install</a></body>
        </html>
        "#;

        let parsed = parse_html_document("https://example.com/docs", html);
        assert_eq!(
            parsed.canonical_url.as_deref(),
            Some("https://example.com/guide")
        );
        assert_eq!(
            parsed.discovered_urls,
            vec!["https://example.com/install".to_string()]
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
        assert_eq!(parsed.robots_directives, RobotsDirectives::default());
    }

    #[test]
    fn html_parser_extracts_meta_robots_directives() {
        let parsed = parse_html_document(
            "https://example.com",
            "<html><head><meta name='robots' content='noindex, nofollow'></head><body>Hello crawler</body></html>",
        );

        assert!(parsed.robots_directives.noindex);
        assert!(parsed.robots_directives.nofollow);
    }

    #[test]
    fn x_robots_tag_parser_respects_generic_and_findverse_scopes() {
        let directives = parse_x_robots_tag_values([
            "noindex",
            "findversebot: nofollow",
            "googlebot: noindex, nofollow",
        ]);

        assert!(directives.noindex);
        assert!(directives.nofollow);
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
    fn html_parser_sanitizes_encoded_html_title() {
        let html = r#"
        <html>
          <head>
            <title>主角座位 - 萌娘百科 万物皆可萌的百科全书</title>
            <meta property="og:title" content="&lt;span class=&quot;mw-page-title-main&quot;&gt;主角座位&lt;/span&gt;" />
          </head>
          <body>
            <template id="MOE_SKIN_TEMPLATE_BODYCONTENT">
              <div id="mw-content-text" class="mw-body-content">
                <div class="mw-parser-output">
                  <p>主角座位，指主角在构图中位于中心的位置。</p>
                </div>
              </div>
            </template>
          </body>
        </html>
        "#;

        let parsed = parse_html_document(
            "https://zh.moegirl.org.cn/%E4%B8%BB%E8%A7%92%E5%BA%A7%E4%BD%8D",
            html,
        );
        assert_eq!(parsed.title.as_deref(), Some("主角座位"));
    }

    #[test]
    fn html_parser_prefers_template_content_and_drops_wiki_noise() {
        let html = r#"
        <html>
          <head>
            <title>filian - 萌娘百科 万物皆可萌的百科全书</title>
            <meta name="description" content="萌娘百科欢迎您参与完善虚拟UP主相关条目☆Kira~ 协助.mw-parser-output .colorededit .Wikiplus-Edit-EveryWhereBtn,...." />
            <meta property="og:title" content="filian" />
            <meta property="og:description" content="萌娘百科欢迎您参与完善虚拟UP主相关条目☆Kira~ 协助.mw-parser-output .colorededit .Wikiplus-Edit-EveryWhereBtn,...." />
          </head>
          <body>
            <noscript>This site requires JavaScript enabled.</noscript>
            <template id="MOE_SKIN_TEMPLATE_BODYCONTENT">
              <div id="mw-content-text" class="mw-body-content">
                <div class="mw-parser-output">
                  <p class="mw-empty-elt">
                    协助
                    <style>.mw-parser-output .colorededit .Wikiplus-Edit-EveryWhereBtn { color: inherit; }</style>
                    <span class="plainlinks colorededit">编辑本条目</span>
                    前，请先阅读萌百编辑简明指南。
                  </p>
                  <p>Filian是活跃于Twitch和YouTube的英语虚拟UP主，以夸张肢体表演和高能直播片段闻名。</p>
                  <div style="display:none!important;">隐藏奖项</div>
                  <table class="navbox"><tr><td>导航模板</td></tr></table>
                </div>
              </div>
            </template>
            <script type="application/json">
              {"title":"filian","footer":"moeskin-footer-top","raw":"RLSTATE=bad"}
            </script>
          </body>
        </html>
        "#;

        let parsed = parse_html_document("https://zh.moegirl.org.cn/%E8%8F%B2%E8%8E%B2", html);
        let body = parsed.body.unwrap();
        let snippet = parsed.snippet.unwrap();

        assert_eq!(parsed.title.as_deref(), Some("filian"));
        assert!(body.contains("Filian是活跃于Twitch和YouTube的英语虚拟UP主"));
        assert!(!body.contains("编辑本条目"));
        assert!(!body.contains("导航模板"));
        assert!(!body.contains("隐藏奖项"));
        assert!(!snippet.contains("欢迎您参与完善"));
        assert!(snippet.contains("Filian是活跃于Twitch和YouTube的英语虚拟UP主"));
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
