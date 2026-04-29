use std::sync::{Arc, OnceLock, RwLock};

use anyhow::{Context, Result};

use crate::models::{SiteRuleBundle, SiteRuleFile};

use super::rules::RuleRegistry;

static ACTIVE_RULES: OnceLock<RwLock<RuleCache>> = OnceLock::new();

macro_rules! embedded_rule_files {
    ($base:literal; [$($name:literal),* $(,)?]) => {
        vec![
            $(
                SiteRuleFile {
                    name: $name.to_string(),
                    content: include_str!(concat!($base, $name)).to_string(),
                }
            ),*
        ]
    };
}

struct RuleCache {
    fingerprint: Option<String>,
    registry: Arc<RuleRegistry>,
}

impl Default for RuleCache {
    fn default() -> Self {
        load_embedded_default_rule_cache().expect("load embedded crawler site rules")
    }
}

pub fn load_active_registry() -> Arc<RuleRegistry> {
    let cache = ACTIVE_RULES.get_or_init(|| RwLock::new(RuleCache::default()));
    let guard = cache.read().expect("site profile cache poisoned");
    Arc::clone(&guard.registry)
}

pub fn install_rule_bundle(bundle: &SiteRuleBundle) -> Result<()> {
    let fingerprint = fingerprint(bundle)?;
    let cache = ACTIVE_RULES.get_or_init(|| RwLock::new(RuleCache::default()));

    {
        let guard = cache.read().expect("site profile cache poisoned");
        if guard.fingerprint.as_deref() == Some(fingerprint.as_str()) {
            return Ok(());
        }
    }

    let registry = Arc::new(RuleRegistry::from_bundle(bundle)?);
    let mut guard = cache.write().expect("site profile cache poisoned");
    guard.fingerprint = Some(fingerprint);
    guard.registry = registry;
    Ok(())
}

#[cfg(test)]
pub(crate) fn load_registry_from_root(root: &std::path::Path) -> Result<RuleRegistry> {
    RuleRegistry::load(
        &root.join("platforms"),
        &root.join("platform-presets"),
        &root.join("sites"),
    )
}

#[cfg(test)]
pub(crate) fn load_registry_from_bundle(bundle: &SiteRuleBundle) -> Result<RuleRegistry> {
    RuleRegistry::from_bundle(bundle)
}

#[cfg(test)]
pub(crate) fn load_embedded_default_registry() -> Result<RuleRegistry> {
    RuleRegistry::from_bundle(&load_embedded_default_rule_bundle()?)
}

fn load_embedded_default_rule_cache() -> Result<RuleCache> {
    let bundle = load_embedded_default_rule_bundle()?;
    Ok(RuleCache {
        fingerprint: Some(fingerprint(&bundle)?),
        registry: Arc::new(RuleRegistry::from_bundle(&bundle)?),
    })
}

fn load_embedded_default_rule_bundle() -> Result<SiteRuleBundle> {
    Ok(SiteRuleBundle {
        platforms: embedded_rule_files!(
            "../../../api/site_rules/platforms/";
            [
                "bitbucket.toml",
                "bookstack.toml",
                "confluence.toml",
                "discourse.toml",
                "docsify.toml",
                "docusaurus.toml",
                "drupal.toml",
                "flarum.toml",
                "forgejo.toml",
                "ghost.toml",
                "gitea.toml",
                "gitee.toml",
                "gitbook.toml",
                "gitlab.toml",
                "hexo.toml",
                "hugo.toml",
                "jekyll.toml",
                "jira.toml",
                "mdbook.toml",
                "mediawiki.toml",
                "mkdocs.toml",
                "nextra.toml",
                "notion.toml",
                "sphinx.toml",
                "typecho.toml",
                "vitepress.toml",
                "vuepress.toml",
                "wikijs.toml",
                "wordpress.toml"
            ]
        ),
        platform_presets: embedded_rule_files!(
            "../../../api/site_rules/platform-presets/";
            [
                "bitbucket.toml",
                "bookstack.toml",
                "confluence.toml",
                "discourse.toml",
                "docsify.toml",
                "docusaurus.toml",
                "drupal.toml",
                "flarum.toml",
                "forgejo.toml",
                "ghost.toml",
                "gitea.toml",
                "gitee.toml",
                "gitbook.toml",
                "gitlab.toml",
                "hexo.toml",
                "hugo.toml",
                "jekyll.toml",
                "jira.toml",
                "mdbook.toml",
                "mediawiki.toml",
                "mkdocs.toml",
                "nextra.toml",
                "notion.toml",
                "sphinx.toml",
                "typecho.toml",
                "vitepress.toml",
                "vuepress.toml",
                "wikijs.toml",
                "wordpress.toml"
            ]
        ),
        sites: embedded_rule_files!(
            "../../../api/site_rules/sites/";
            ["github.toml", "readthedocs.toml"]
        ),
    })
}

fn fingerprint(bundle: &SiteRuleBundle) -> Result<String> {
    serde_json::to_string(bundle).context("serialize site rule bundle fingerprint")
}
