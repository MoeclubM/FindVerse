#[cfg(feature = "js-render")]
use headless_chrome::{Browser, LaunchOptions};
#[cfg(feature = "js-render")]
use std::time::Duration;

pub fn needs_js_rendering(html: &str, body_text: &str) -> bool {
    let body_text = body_text.trim();
    let has_content_root = [
        "<article",
        "<main",
        "role=\"main\"",
        "itemprop=\"articleBody\"",
        "class=\"article-content\"",
        "class=\"entry-content\"",
        "class=\"post-content\"",
        "class=\"article-body\"",
        "id=\"content\"",
        "id=\"main-content\"",
    ]
    .iter()
    .any(|marker| html.contains(marker));
    let has_shell_marker = [
        "data-reactroot",
        "ng-app",
        "id=\"app\"",
        "id=\"root\"",
        "data-svelte",
        "data-sveltekit",
    ]
    .iter()
    .any(|marker| html.contains(marker));

    if body_text.len() >= 600 || (has_content_root && body_text.len() >= 120) {
        return false;
    }

    if has_shell_marker {
        return true;
    }

    body_text.len() < 40 && html.contains("<script") && !has_content_root
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
    fn server_rendered_pages_do_not_force_js_rendering() {
        let html = r#"
            <html>
              <head>
                <script src="/load.php"></script>
              </head>
              <body>
                <main id="content">
                  <article class="entry-content">
                    <p>Example article body.</p>
                  </article>
                </main>
              </body>
            </html>
        "#;

        assert!(!needs_js_rendering(
            html,
            "Example article body with enough static content to skip browser rendering."
        ));
    }

    #[test]
    fn short_static_pages_with_content_root_do_not_force_js_rendering() {
        let html = r#"
            <html>
              <head><script src="/assets/main.js"></script></head>
              <body>
                <main>
                  <p>About us.</p>
                </main>
              </body>
            </html>
        "#;

        assert!(!needs_js_rendering(html, "About us."));
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
