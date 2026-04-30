#![allow(dead_code)]

use std::io::Read;

use anyhow::Result;
use quick_xml::Reader;
use quick_xml::events::{BytesRef, Event};
use tracing::info;
use url::Url;

const MAX_SITEMAP_DEPTH: u8 = 2;

#[derive(Debug, Clone)]
pub struct SitemapEntry {
    pub url: String,
    pub lastmod: Option<String>,
    pub changefreq: Option<String>,
    pub priority: Option<f32>,
}

pub async fn fetch_and_parse_sitemap(
    client: &reqwest::Client,
    sitemap_url: &str,
) -> Result<Vec<SitemapEntry>> {
    fetch_sitemap_recursive(client, sitemap_url, 0).await
}

/// Convenience helper: returns just URLs for callers that don't need metadata.
pub fn sitemap_urls(entries: &[SitemapEntry]) -> Vec<String> {
    entries.iter().map(|e| e.url.clone()).collect()
}

fn fetch_sitemap_recursive<'a>(
    client: &'a reqwest::Client,
    url: &'a str,
    depth: u8,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<SitemapEntry>>> + Send + 'a>> {
    Box::pin(async move {
        if depth > MAX_SITEMAP_DEPTH {
            return Ok(vec![]);
        }

        let resp = client.get(url).send().await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }
        let bytes = resp.bytes().await?;

        // Decompress gzip if needed
        let body = decompress_if_gzip(&bytes)?;

        if body.contains("<sitemapindex") {
            let child_locs = parse_sitemap_index_xml(&body);
            let mut all_entries = Vec::new();
            for child_url in child_locs.into_iter().take(20) {
                info!("following sitemap index entry: {}", child_url);
                if let Ok(entries) = fetch_sitemap_recursive(client, &child_url, depth + 1).await {
                    all_entries.extend(entries);
                }
            }
            Ok(all_entries)
        } else if body.contains("<rss") || body.contains("<feed") {
            Ok(parse_feed_xml(&body))
        } else {
            Ok(parse_urlset_xml(&body))
        }
    })
}

fn decompress_if_gzip(bytes: &[u8]) -> Result<String> {
    if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
        let mut decoder = flate2::read::GzDecoder::new(bytes);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed)?;
        Ok(decompressed)
    } else {
        Ok(String::from_utf8_lossy(bytes).to_string())
    }
}

/// Tag currently being read inside a `<url>` element.
#[derive(Clone, Copy, PartialEq, Eq)]
enum UrlTag {
    Loc,
    Lastmod,
    Changefreq,
    Priority,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FeedTag {
    Link,
    Updated,
    Published,
    PubDate,
    Other,
}

/// Resolves a quick-xml general entity reference (e.g. `&amp;`, `&#x30;`) into
/// its textual representation. Recognises the five predefined XML entities and
/// numeric character references; unknown entities are returned as the empty
/// string so they are silently dropped (matching the prior `unescape()` lossy
/// behaviour for malformed input).
fn resolve_entity_ref(e: &BytesRef<'_>) -> String {
    if let Ok(Some(ch)) = e.resolve_char_ref() {
        return ch.to_string();
    }
    match e.decode().ok().as_deref() {
        Some("amp") => "&".to_string(),
        Some("lt") => "<".to_string(),
        Some("gt") => ">".to_string(),
        Some("quot") => "\"".to_string(),
        Some("apos") => "'".to_string(),
        _ => String::new(),
    }
}

fn parse_urlset_xml(xml: &str) -> Vec<SitemapEntry> {
    let mut reader = Reader::from_str(xml);
    let mut entries = Vec::new();

    let mut inside_url = false;
    let mut current_tag = UrlTag::Other;
    let mut loc = String::new();
    let mut lastmod = String::new();
    let mut changefreq = String::new();
    let mut priority_str = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"url" => {
                        inside_url = true;
                        loc.clear();
                        lastmod.clear();
                        changefreq.clear();
                        priority_str.clear();
                    }
                    b"loc" if inside_url => current_tag = UrlTag::Loc,
                    b"lastmod" if inside_url => current_tag = UrlTag::Lastmod,
                    b"changefreq" if inside_url => current_tag = UrlTag::Changefreq,
                    b"priority" if inside_url => current_tag = UrlTag::Priority,
                    _ => current_tag = UrlTag::Other,
                }
            }
            Ok(Event::Text(ref e)) if inside_url => {
                if let Ok(text) = e.decode() {
                    match current_tag {
                        UrlTag::Loc => loc.push_str(&text),
                        UrlTag::Lastmod => lastmod.push_str(&text),
                        UrlTag::Changefreq => changefreq.push_str(&text),
                        UrlTag::Priority => priority_str.push_str(&text),
                        UrlTag::Other => {}
                    }
                }
            }
            Ok(Event::GeneralRef(ref e)) if inside_url => {
                let text = resolve_entity_ref(e);
                match current_tag {
                    UrlTag::Loc => loc.push_str(&text),
                    UrlTag::Lastmod => lastmod.push_str(&text),
                    UrlTag::Changefreq => changefreq.push_str(&text),
                    UrlTag::Priority => priority_str.push_str(&text),
                    UrlTag::Other => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                if name.as_ref() == b"url" && inside_url {
                    let trimmed_loc = loc.trim().to_string();
                    if Url::parse(&trimmed_loc).is_ok() {
                        entries.push(SitemapEntry {
                            url: trimmed_loc,
                            lastmod: non_empty(lastmod.trim()),
                            changefreq: non_empty(changefreq.trim()),
                            priority: priority_str.trim().parse::<f32>().ok(),
                        });
                    }
                    inside_url = false;
                }
                current_tag = UrlTag::Other;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    entries
}

fn parse_sitemap_index_xml(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    let mut urls = Vec::new();

    let mut inside_sitemap = false;
    let mut inside_loc = false;
    let mut loc = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"sitemap" => {
                        inside_sitemap = true;
                        loc.clear();
                    }
                    b"loc" if inside_sitemap => inside_loc = true,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if inside_loc => {
                if let Ok(text) = e.decode() {
                    loc.push_str(&text);
                }
            }
            Ok(Event::GeneralRef(ref e)) if inside_loc => {
                loc.push_str(&resolve_entity_ref(e));
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"loc" => inside_loc = false,
                    b"sitemap" if inside_sitemap => {
                        let trimmed = loc.trim().to_string();
                        if Url::parse(&trimmed).is_ok() {
                            urls.push(trimmed);
                        }
                        inside_sitemap = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    urls
}

fn parse_feed_xml(xml: &str) -> Vec<SitemapEntry> {
    let mut reader = Reader::from_str(xml);
    let mut entries = Vec::new();

    let mut inside_entry = false;
    let mut current_tag = FeedTag::Other;
    let mut loc = String::new();
    let mut lastmod = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"item" | b"entry" => {
                        inside_entry = true;
                        loc.clear();
                        lastmod.clear();
                        current_tag = FeedTag::Other;
                    }
                    b"link" if inside_entry => {
                        current_tag = FeedTag::Link;
                        if loc.is_empty() {
                            for attr in e.attributes().flatten() {
                                if attr.key.as_ref() == b"href" {
                                    loc = String::from_utf8_lossy(attr.value.as_ref())
                                        .trim()
                                        .to_string();
                                    break;
                                }
                            }
                        }
                    }
                    b"updated" if inside_entry => current_tag = FeedTag::Updated,
                    b"published" if inside_entry => current_tag = FeedTag::Published,
                    b"pubDate" if inside_entry => current_tag = FeedTag::PubDate,
                    _ => current_tag = FeedTag::Other,
                }
            }
            Ok(Event::Empty(ref e)) if inside_entry => {
                let name = e.local_name();
                if name.as_ref() == b"link" && loc.is_empty() {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"href" {
                            loc = String::from_utf8_lossy(attr.value.as_ref())
                                .trim()
                                .to_string();
                            break;
                        }
                    }
                }
                current_tag = FeedTag::Other;
            }
            Ok(Event::Text(ref e)) if inside_entry => {
                if let Ok(text) = e.decode() {
                    let text = text.trim();
                    match current_tag {
                        FeedTag::Link => {
                            if loc.is_empty() {
                                loc.push_str(text);
                            }
                        }
                        FeedTag::Updated | FeedTag::Published | FeedTag::PubDate => {
                            if lastmod.is_empty() {
                                lastmod.push_str(text);
                            }
                        }
                        FeedTag::Other => {}
                    }
                }
            }
            Ok(Event::GeneralRef(ref e)) if inside_entry => {
                let text = resolve_entity_ref(e);
                match current_tag {
                    FeedTag::Link => loc.push_str(&text),
                    FeedTag::Updated | FeedTag::Published | FeedTag::PubDate => {
                        lastmod.push_str(&text);
                    }
                    FeedTag::Other => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.local_name();
                if matches!(name.as_ref(), b"item" | b"entry") && inside_entry {
                    let trimmed_loc = loc.trim().to_string();
                    if Url::parse(&trimmed_loc).is_ok() {
                        entries.push(SitemapEntry {
                            url: trimmed_loc,
                            lastmod: non_empty(lastmod.trim()),
                            changefreq: None,
                            priority: None,
                        });
                    }
                    inside_entry = false;
                }
                current_tag = FeedTag::Other;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    entries
}

fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_urlset() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/page1</loc>
    <lastmod>2026-01-15</lastmod>
    <changefreq>weekly</changefreq>
    <priority>0.8</priority>
  </url>
  <url>
    <loc>https://example.com/page2</loc>
  </url>
</urlset>"#;

        let entries = parse_urlset_xml(xml);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url, "https://example.com/page1");
        assert_eq!(entries[0].lastmod.as_deref(), Some("2026-01-15"));
        assert_eq!(entries[0].changefreq.as_deref(), Some("weekly"));
        assert_eq!(entries[0].priority, Some(0.8));
        assert_eq!(entries[1].url, "https://example.com/page2");
        assert!(entries[1].lastmod.is_none());
        assert!(entries[1].changefreq.is_none());
        assert!(entries[1].priority.is_none());
    }

    #[test]
    fn test_parse_sitemap_index() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap>
    <loc>https://example.com/sitemap-posts.xml</loc>
    <lastmod>2026-03-01</lastmod>
  </sitemap>
  <sitemap>
    <loc>https://example.com/sitemap-pages.xml</loc>
  </sitemap>
</sitemapindex>"#;

        assert!(xml.contains("<sitemapindex"));
        let child_urls = parse_sitemap_index_xml(xml);
        assert_eq!(child_urls.len(), 2);
        assert_eq!(child_urls[0], "https://example.com/sitemap-posts.xml");
        assert_eq!(child_urls[1], "https://example.com/sitemap-pages.xml");
    }

    #[test]
    fn test_gzip_decompression() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/gzipped</loc></url>
</urlset>"#;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(xml.as_bytes()).unwrap();
        let compressed = encoder.finish().unwrap();

        // Verify magic bytes
        assert_eq!(compressed[0], 0x1f);
        assert_eq!(compressed[1], 0x8b);

        let decompressed = decompress_if_gzip(&compressed).unwrap();
        let entries = parse_urlset_xml(&decompressed);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].url, "https://example.com/gzipped");
    }

    #[test]
    fn test_non_gzip_passthrough() {
        let plain = b"<urlset><url><loc>https://example.com/plain</loc></url></urlset>";
        let result = decompress_if_gzip(plain).unwrap();
        assert!(result.contains("https://example.com/plain"));
    }

    #[test]
    fn test_multiline_loc() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>
      https://example.com/multiline
    </loc>
  </url>
</urlset>"#;

        let entries = parse_urlset_xml(xml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].url, "https://example.com/multiline");
    }

    #[test]
    fn test_malformed_xml_graceful() {
        let xml = r#"<urlset>
  <url><loc>https://example.com/ok</loc></url>
  <url><loc>not-a-url</loc></url>
  <url><loc>https://example.com/also-ok</loc></url>
  <broken
</urlset>"#;

        let entries = parse_urlset_xml(xml);
        // Should parse what it can before hitting the broken element
        assert!(entries.len() >= 2);
        assert_eq!(entries[0].url, "https://example.com/ok");
        assert_eq!(entries[1].url, "https://example.com/also-ok");
    }

    #[test]
    fn test_empty_xml() {
        let entries = parse_urlset_xml("");
        assert!(entries.is_empty());

        let urls = parse_sitemap_index_xml("");
        assert!(urls.is_empty());
    }

    #[test]
    fn test_parse_rss_feed() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <item>
      <title>Example</title>
      <link>https://example.com/post-1</link>
      <pubDate>Mon, 01 Apr 2026 00:00:00 GMT</pubDate>
    </item>
  </channel>
</rss>"#;

        let entries = parse_feed_xml(xml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].url, "https://example.com/post-1");
        assert_eq!(
            entries[0].lastmod.as_deref(),
            Some("Mon, 01 Apr 2026 00:00:00 GMT")
        );
    }

    #[test]
    fn test_parse_atom_feed() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <title>Example</title>
    <link href="https://example.com/post-2" />
    <updated>2026-04-01T00:00:00Z</updated>
  </entry>
</feed>"#;

        let entries = parse_feed_xml(xml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].url, "https://example.com/post-2");
        assert_eq!(entries[0].lastmod.as_deref(), Some("2026-04-01T00:00:00Z"));
    }

    #[test]
    fn test_sitemap_urls_helper() {
        let entries = vec![
            SitemapEntry {
                url: "https://a.com".to_string(),
                lastmod: None,
                changefreq: None,
                priority: None,
            },
            SitemapEntry {
                url: "https://b.com".to_string(),
                lastmod: Some("2026-01-01".to_string()),
                changefreq: None,
                priority: Some(0.5),
            },
        ];
        let urls = sitemap_urls(&entries);
        assert_eq!(urls, vec!["https://a.com", "https://b.com"]);
    }
}
