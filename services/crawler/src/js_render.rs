#[cfg(feature = "js-render")]
use headless_chrome::{Browser, LaunchOptions};
#[cfg(feature = "js-render")]
use std::time::Duration;

pub fn needs_js_rendering(html: &str, body_text: &str) -> bool {
    if looks_like_server_rendered_mediawiki(html) {
        return false;
    }

    // Heuristic: if body is very short but HTML has script tags, likely needs JS
    if body_text.len() < 200 && html.contains("<script") {
        return true;
    }

    // Check for common SPA frameworks (client-side rendered only)
    html.contains("data-reactroot")
        || html.contains("ng-app")
        || html.contains("id=\"app\"")
        || html.contains("id=\"root\"")
        || html.contains("data-svelte")
        || html.contains("data-sveltekit")
}

fn looks_like_server_rendered_mediawiki(html: &str) -> bool {
    html.contains("content=\"MediaWiki")
        || html.contains("id=\"mw-content-text\"")
        || html.contains("class=\"mw-parser-output\"")
        || html.contains("class=\"mw-page-title-main\"")
        || html.contains("class=\"mw-body-content\"")
}

#[cfg(feature = "js-render")]
pub async fn render_with_js(url: &str) -> anyhow::Result<String> {
    let url = url.to_string();
    tokio::task::spawn_blocking(move || {
        let browser = Browser::new(LaunchOptions {
            headless: true,
            sandbox: false,
            ..Default::default()
        })?;

        let tab = browser.new_tab()?;
        tab.navigate_to(&url)?;
        tab.wait_for_element("body")?;

        // Wait for JS to execute (blocking is OK inside spawn_blocking)
        std::thread::sleep(Duration::from_secs(2));

        let html = tab.get_content()?;
        Ok(html)
    })
    .await?
}

#[cfg(not(feature = "js-render"))]
pub async fn render_with_js(_url: &str) -> anyhow::Result<String> {
    anyhow::bail!("JS rendering not enabled (compile with --features js-render)")
}

#[cfg(test)]
mod tests {
    use super::needs_js_rendering;

    #[test]
    fn mediawiki_pages_do_not_force_js_rendering() {
        let html = r#"
            <html>
              <head>
                <meta name="generator" content="MediaWiki 1.43.0">
                <script src="/load.php"></script>
              </head>
              <body>
                <main id="mw-content-text">
                  <div class="mw-parser-output">
                    <p>Example article body.</p>
                  </div>
                </main>
              </body>
            </html>
        "#;

        assert!(!needs_js_rendering(html, "Example article body."));
    }

    #[test]
    fn spa_shells_still_trigger_js_rendering() {
        let html = r#"
            <html>
              <head><script src="/assets/index.js"></script></head>
              <body><div id="root"></div></body>
            </html>
        "#;

        assert!(needs_js_rendering(html, ""));
    }
}
