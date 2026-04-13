mod loader;
mod rules;

use anyhow::Result;

use crate::models::SiteRuleBundle;

pub use rules::PageAction;

#[derive(Debug, Clone)]
pub struct SiteProfile {
    preset: rules::EffectivePreset,
}

impl SiteProfile {
    pub fn detect(url: &str, html: &str) -> Result<Self> {
        let registry = loader::load_active_registry();
        Ok(Self {
            preset: registry.detect(url, html)?,
        })
    }

    pub fn prefers_js_render(&self) -> bool {
        self.preset.prefer_js
    }

    pub fn page_action(&self, url: &str) -> PageAction {
        self.preset.page_decision(url).action
    }

    pub fn filtered_reason(&self, url: &str) -> Option<String> {
        let decision = self.preset.page_decision(url);
        (decision.action == PageAction::Deny).then(|| {
            decision
                .reason
                .unwrap_or_else(|| format!("page skipped by {} preset", self.preset.id))
        })
    }

    pub fn allows_discovery(&self, url: &str) -> bool {
        self.page_action(url).allows_discovery()
    }

    pub fn discovery_sources(&self, origin: &str) -> Vec<String> {
        self.preset.discovery_sources(origin)
    }
}

pub fn install_rule_bundle(bundle: &SiteRuleBundle) -> Result<()> {
    loader::install_rule_bundle(bundle)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::models::{SiteRuleBundle, SiteRuleFile};

    use super::{PageAction, SiteProfile, loader};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("findverse-{name}-{suffix}"));
        fs::create_dir_all(root.join("platforms")).expect("create platforms directory");
        fs::create_dir_all(root.join("platform-presets"))
            .expect("create platform-presets directory");
        fs::create_dir_all(root.join("sites")).expect("create sites directory");
        root
    }

    fn write_rule_file(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("rule parent"))
            .expect("create rule parent directory");
        fs::write(path, content).expect("write rule file");
    }

    #[test]
    fn site_preset_can_override_platform_preset() {
        let root = unique_temp_dir("site-profile-override");
        write_rule_file(
            &root,
            "platforms/wordpress.toml",
            r#"
id = "wordpress"
preset = "wordpress"

[match]
html_markers = ["wp-content"]
"#,
        );
        write_rule_file(
            &root,
            "platform-presets/wordpress.toml",
            r#"
id = "wordpress"
default_action = "allow_index_discover"

[[rules]]
name = "wp-admin"
action = "deny"
path_prefixes = ["/wp-admin"]
"#,
        );
        write_rule_file(
            &root,
            "sites/docs-example.toml",
            r#"
id = "docs-example"
extends = "wordpress"
default_action = "deny"

[match]
hosts = ["docs.example.com"]

[[rules]]
name = "home"
action = "allow_index_discover"
path_regex = "^/$"
"#,
        );

        let registry = loader::load_registry_from_root(&root).expect("load registry");
        let profile = SiteProfile {
            preset: registry
                .detect(
                    "https://docs.example.com/",
                    r#"<html><body><link href="/wp-content/theme.css"></body></html>"#,
                )
                .expect("detect profile"),
        };

        assert_eq!(profile.page_action("https://docs.example.com/"), PageAction::AllowIndexDiscover);
        assert_eq!(
            profile.page_action("https://docs.example.com/wp-admin/edit.php"),
            PageAction::Deny
        );
    }

    #[test]
    fn github_readme_is_index_only_but_code_file_is_denied() {
        let registry = loader::load_registry_from_bundle(&SiteRuleBundle {
            platforms: Vec::new(),
            platform_presets: Vec::new(),
            sites: vec![SiteRuleFile {
                name: "github.toml".to_string(),
                content: r#"
id = "github"
priority = 100
default_action = "deny"

[match]
hosts = ["github.com"]

[[rules]]
name = "repo-root"
action = "allow_index_discover"
path_regex = "^/[^/]+/[^/]+/?$"

[[rules]]
name = "repo-readme"
action = "allow_index_only"
path_regex = "^/[^/]+/[^/]+/blob/[^/]+/README(\\.[A-Za-z0-9._-]+)?$"

[[rules]]
name = "repo-code"
action = "deny"
path_regex = "^/[^/]+/[^/]+/blob/"
"#
                .to_string(),
            }],
        })
        .expect("load registry");
        let profile = SiteProfile {
            preset: registry
                .detect("https://github.com/owner/repo", "<html><body></body></html>")
                .expect("detect profile"),
        };

        assert_eq!(
            profile.page_action("https://github.com/owner/repo"),
            PageAction::AllowIndexDiscover
        );
        assert_eq!(
            profile.page_action("https://github.com/owner/repo/blob/main/README.md"),
            PageAction::AllowIndexOnly
        );
        assert_eq!(
            profile.page_action("https://github.com/owner/repo/blob/main/src/main.rs"),
            PageAction::Deny
        );
        assert!(profile
            .filtered_reason("https://github.com/owner/repo/blob/main/src/main.rs")
            .is_some());
    }

    #[test]
    fn falls_back_to_full_crawl_when_no_rule_matches() {
        let root = unique_temp_dir("site-profile-fallback");
        let registry = loader::load_registry_from_root(&root).expect("load registry");
        let profile = SiteProfile {
            preset: registry
                .detect("https://unknown.example.com/docs/page", "<html><body>plain html</body></html>")
                .expect("detect profile"),
        };

        assert_eq!(
            profile.page_action("https://unknown.example.com/docs/page"),
            PageAction::AllowIndexDiscover
        );
        assert!(profile
            .filtered_reason("https://unknown.example.com/docs/page")
            .is_none());
        assert!(profile.allows_discovery("https://unknown.example.com/docs/page"));
    }
}
