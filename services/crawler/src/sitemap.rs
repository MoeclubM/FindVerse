use anyhow::Result;
use tracing::info;
use url::Url;

const MAX_SITEMAP_DEPTH: u8 = 2;

pub async fn fetch_and_parse_sitemap(
    client: &reqwest::Client,
    sitemap_url: &str,
) -> Result<Vec<String>> {
    fetch_sitemap_recursive(client, sitemap_url, 0).await
}

fn fetch_sitemap_recursive<'a>(
    client: &'a reqwest::Client,
    url: &'a str,
    depth: u8,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<String>>> + Send + 'a>> {
    Box::pin(async move {
        if depth > MAX_SITEMAP_DEPTH {
            return Ok(vec![]);
        }

        let resp = client.get(url).send().await?;
        if !resp.status().is_success() {
            return Ok(vec![]);
        }
        let body = resp.text().await?;

        // Check if this is a sitemap index (contains <sitemapindex>)
        if body.contains("<sitemapindex") {
            let child_urls = parse_sitemap_xml(&body);
            let mut all_urls = Vec::new();
            for child_url in child_urls.into_iter().take(20) {
                info!("following sitemap index entry: {}", child_url);
                if let Ok(urls) = fetch_sitemap_recursive(client, &child_url, depth + 1).await {
                    all_urls.extend(urls);
                }
            }
            Ok(all_urls)
        } else {
            Ok(parse_sitemap_xml(&body))
        }
    })
}

fn parse_sitemap_xml(xml: &str) -> Vec<String> {
    let mut urls = Vec::new();

    for line in xml.lines() {
        let trimmed = line.trim();
        if let Some(start) = trimmed.find("<loc>") {
            if let Some(end) = trimmed.find("</loc>") {
                let url = &trimmed[start + 5..end];
                if Url::parse(url).is_ok() {
                    urls.push(url.to_string());
                }
            }
        }
    }

    urls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sitemap() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/page1</loc>
  </url>
  <url>
    <loc>https://example.com/page2</loc>
  </url>
</urlset>"#;

        let urls = parse_sitemap_xml(xml);
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"https://example.com/page1".to_string()));
    }

    #[test]
    fn test_parse_sitemap_index() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap>
    <loc>https://example.com/sitemap-posts.xml</loc>
  </sitemap>
  <sitemap>
    <loc>https://example.com/sitemap-pages.xml</loc>
  </sitemap>
</sitemapindex>"#;

        // Should detect as sitemap index
        assert!(xml.contains("<sitemapindex"));
        let child_urls = parse_sitemap_xml(xml);
        assert_eq!(child_urls.len(), 2);
    }
}
