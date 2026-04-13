use std::sync::{Arc, OnceLock, RwLock};

use anyhow::{Context, Result};

use crate::models::SiteRuleBundle;

use super::rules::RuleRegistry;

static ACTIVE_RULES: OnceLock<RwLock<RuleCache>> = OnceLock::new();

struct RuleCache {
    fingerprint: Option<String>,
    registry: Arc<RuleRegistry>,
}

impl Default for RuleCache {
    fn default() -> Self {
        Self {
            fingerprint: None,
            registry: Arc::new(RuleRegistry::empty()),
        }
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

fn fingerprint(bundle: &SiteRuleBundle) -> Result<String> {
    serde_json::to_string(bundle).context("serialize site rule bundle fingerprint")
}
