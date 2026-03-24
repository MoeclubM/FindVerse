#[cfg(feature = "js-render")]
use headless_chrome::{Browser, LaunchOptions};
#[cfg(feature = "js-render")]
use std::time::Duration;

pub fn needs_js_rendering(html: &str, body_text: &str) -> bool {
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

#[cfg(feature = "js-render")]
pub async fn render_with_js(url: &str) -> anyhow::Result<String> {
    let url = url.to_string();
    tokio::task::spawn_blocking(move || {
        let browser = Browser::new(LaunchOptions {
            headless: true,
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
