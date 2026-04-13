use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Context, anyhow};

use crate::{
    error::ApiError,
    models::{SiteRuleBundle, SiteRuleFile},
};

pub const SITE_RULE_BUNDLE_CONFIG_KEY: &str = "crawler.site_rules_bundle";

static DEFAULT_SITE_RULE_BUNDLE: OnceLock<Result<SiteRuleBundle, String>> = OnceLock::new();

pub fn normalize_site_rule_bundle_json(input: &str) -> Result<String, ApiError> {
    let bundle: SiteRuleBundle = serde_json::from_str(input).map_err(|error| {
        ApiError::BadRequest(format!(
            "{SITE_RULE_BUNDLE_CONFIG_KEY} must be valid json: {error}"
        ))
    })?;
    serde_json::to_string(&bundle).map_err(|error| ApiError::Internal(error.into()))
}

pub fn resolve_site_rule_bundle(config_value: Option<&str>) -> Result<SiteRuleBundle, ApiError> {
    if let Some(raw) = config_value {
        return serde_json::from_str(raw).map_err(|error| {
            ApiError::Internal(anyhow!(
                "invalid stored {SITE_RULE_BUNDLE_CONFIG_KEY}: {error}"
            ))
        });
    }

    match DEFAULT_SITE_RULE_BUNDLE
        .get_or_init(|| load_default_site_rule_bundle().map_err(|error| error.to_string()))
    {
        Ok(bundle) => Ok(bundle.clone()),
        Err(message) => Err(ApiError::Internal(anyhow!(message.clone()))),
    }
}

fn load_default_site_rule_bundle() -> anyhow::Result<SiteRuleBundle> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("site_rules");
    Ok(SiteRuleBundle {
        platforms: read_rule_files(&root.join("platforms"))?,
        platform_presets: read_rule_files(&root.join("platform-presets"))?,
        sites: read_rule_files(&root.join("sites"))?,
    })
}

fn read_rule_files(directory: &Path) -> anyhow::Result<Vec<SiteRuleFile>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)
        .with_context(|| format!("read site rules directory {}", directory.display()))?
    {
        let entry = entry.with_context(|| format!("read entry in {}", directory.display()))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| anyhow!("invalid site rule filename {}", path.display()))?
            .to_string();
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        files.push(SiteRuleFile { name, content });
    }
    files.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(files)
}
