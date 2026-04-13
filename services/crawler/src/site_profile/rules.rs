use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde::Deserialize;
use url::Url;

use crate::models::{SiteRuleBundle, SiteRuleFile};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageAction {
    AllowIndexDiscover,
    AllowIndexOnly,
    Deny,
}

impl Default for PageAction {
    fn default() -> Self {
        Self::AllowIndexDiscover
    }
}

impl PageAction {
    pub fn allows_discovery(self) -> bool {
        matches!(self, Self::AllowIndexDiscover)
    }
}

#[derive(Debug, Clone)]
pub struct PageDecision {
    pub action: PageAction,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EffectivePreset {
    pub id: String,
    pub prefer_js: bool,
    pub default_action: PageAction,
    discovery_sources: Vec<String>,
    rules: Vec<CompiledPageRule>,
}

impl EffectivePreset {
    pub fn unknown() -> Self {
        Self {
            id: "unknown".to_string(),
            prefer_js: false,
            default_action: PageAction::AllowIndexDiscover,
            discovery_sources: Vec::new(),
            rules: Vec::new(),
        }
    }

    pub fn page_decision(&self, url: &str) -> PageDecision {
        let Ok(parsed) = Url::parse(url) else {
            return PageDecision {
                action: self.default_action,
                reason: None,
            };
        };

        for rule in &self.rules {
            if rule.matches(&parsed) {
                return PageDecision {
                    action: rule.action,
                    reason: rule.reason.clone(),
                };
            }
        }

        PageDecision {
            action: self.default_action,
            reason: (self.default_action == PageAction::Deny)
                .then(|| format!("page skipped by {} preset", self.id)),
        }
    }

    pub fn discovery_sources(&self, origin: &str) -> Vec<String> {
        let mut sources = BTreeSet::new();
        for source in &self.discovery_sources {
            let resolved = if source.contains("{origin}") {
                source.replace("{origin}", origin)
            } else if source.starts_with("https://") || source.starts_with("http://") {
                source.clone()
            } else if source.starts_with('/') {
                format!("{origin}{source}")
            } else {
                format!("{origin}/{source}")
            };
            sources.insert(resolved);
        }
        sources.into_iter().collect()
    }

    fn apply_override(mut self, override_preset: PresetOverride) -> Self {
        if let Some(prefer_js) = override_preset.prefer_js {
            self.prefer_js = prefer_js;
        }
        if let Some(default_action) = override_preset.default_action {
            self.default_action = default_action;
        }
        self.id = override_preset.id;
        self.rules.splice(0..0, override_preset.rules);
        for source in override_preset.discovery_sources {
            if !self.discovery_sources.contains(&source) {
                self.discovery_sources.push(source);
            }
        }
        self
    }
}

#[derive(Debug, Clone)]
pub struct RuleRegistry {
    platform_identifiers: Vec<PlatformIdentifier>,
    platform_presets: BTreeMap<String, EffectivePreset>,
    site_presets: Vec<SitePreset>,
}

impl RuleRegistry {
    pub fn empty() -> Self {
        Self {
            platform_identifiers: Vec::new(),
            platform_presets: BTreeMap::new(),
            site_presets: Vec::new(),
        }
    }

    pub fn load(
        platform_dir: &Path,
        platform_preset_dir: &Path,
        site_dir: &Path,
    ) -> Result<Self> {
        let platform_presets = load_platform_presets(platform_preset_dir)?;
        let mut platform_identifiers = load_platform_identifiers(platform_dir)?;
        let mut site_presets = load_site_presets(site_dir)?;

        platform_identifiers.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        site_presets.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        validate_registry(&platform_identifiers, &platform_presets, &site_presets)?;

        Ok(Self {
            platform_identifiers,
            platform_presets,
            site_presets,
        })
    }

    pub fn from_bundle(bundle: &SiteRuleBundle) -> Result<Self> {
        let platform_presets = load_platform_presets_from_bundle(&bundle.platform_presets)?;
        let mut platform_identifiers = load_platform_identifiers_from_bundle(&bundle.platforms)?;
        let mut site_presets = load_site_presets_from_bundle(&bundle.sites)?;

        platform_identifiers.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        site_presets.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        validate_registry(&platform_identifiers, &platform_presets, &site_presets)?;

        Ok(Self {
            platform_identifiers,
            platform_presets,
            site_presets,
        })
    }

    pub fn detect(&self, url: &str, html: &str) -> Result<EffectivePreset> {
        let parsed = Url::parse(url).with_context(|| format!("invalid profile url {url}"))?;
        let normalized_html = html.to_ascii_lowercase();

        let matched_platform = self
            .platform_identifiers
            .iter()
            .find(|identifier| identifier.matcher.matches(&parsed, &normalized_html));
        let matched_site = self
            .site_presets
            .iter()
            .find(|site| site.matcher.matches(&parsed, &normalized_html));

        let mut preset = if let Some(identifier) = matched_platform {
            self.platform_presets
                .get(&identifier.preset)
                .cloned()
                .ok_or_else(|| {
                    anyhow!(
                        "platform {} references missing preset {}",
                        identifier.id,
                        identifier.preset
                    )
                })?
        } else {
            EffectivePreset::unknown()
        };

        if let Some(site) = matched_site {
            if let Some(base) = site.extends.as_deref() {
                preset = self
                    .platform_presets
                    .get(base)
                    .cloned()
                    .ok_or_else(|| anyhow!("site {} extends missing preset {}", site.id, base))?;
            }
            preset = preset.apply_override(site.preset.clone());
        }

        Ok(preset)
    }
}

#[derive(Debug, Clone)]
struct PlatformIdentifier {
    id: String,
    priority: i32,
    preset: String,
    matcher: CompiledMatchRule,
}

#[derive(Debug, Clone)]
struct SitePreset {
    id: String,
    priority: i32,
    extends: Option<String>,
    matcher: CompiledMatchRule,
    preset: PresetOverride,
}

#[derive(Debug, Clone)]
struct PresetOverride {
    id: String,
    prefer_js: Option<bool>,
    default_action: Option<PageAction>,
    discovery_sources: Vec<String>,
    rules: Vec<CompiledPageRule>,
}

#[derive(Debug, Clone, Default)]
struct CompiledMatchRule {
    hosts: Vec<String>,
    host_suffixes: Vec<String>,
    path_prefixes: Vec<String>,
    path_regexes: Vec<Regex>,
    html_markers: Vec<String>,
    html_regexes: Vec<Regex>,
}

impl CompiledMatchRule {
    fn matches(&self, url: &Url, html: &str) -> bool {
        let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
        let path = url.path().to_ascii_lowercase();

        (self.hosts.is_empty() || self.hosts.iter().any(|candidate| host == *candidate))
            && (self.host_suffixes.is_empty()
                || self
                    .host_suffixes
                    .iter()
                    .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}"))))
            && (self.path_prefixes.is_empty()
                || self
                    .path_prefixes
                    .iter()
                    .any(|prefix| path_starts_with_rule(&path, prefix)))
            && (self.path_regexes.is_empty()
                || self.path_regexes.iter().any(|regex| regex.is_match(&path)))
            && (self.html_markers.is_empty()
                || self
                    .html_markers
                    .iter()
                    .any(|marker| html.contains(marker.as_str())))
            && (self.html_regexes.is_empty()
                || self.html_regexes.iter().any(|regex| regex.is_match(html)))
    }
}

#[derive(Debug, Clone)]
struct CompiledPageRule {
    action: PageAction,
    reason: Option<String>,
    hosts: Vec<String>,
    host_suffixes: Vec<String>,
    path_exacts: Vec<String>,
    path_prefixes: Vec<String>,
    path_regex: Option<Regex>,
    query_keys: Vec<String>,
    query_values: Vec<(String, String)>,
    query_value_prefixes: Vec<(String, String)>,
}

impl CompiledPageRule {
    fn matches(&self, url: &Url) -> bool {
        let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
        let path = url.path().to_ascii_lowercase();

        (self.hosts.is_empty() || self.hosts.iter().any(|candidate| host == *candidate))
            && (self.host_suffixes.is_empty()
                || self
                    .host_suffixes
                    .iter()
                    .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}"))))
            && (self.path_exacts.is_empty() || self.path_exacts.iter().any(|exact| path == *exact))
            && (self.path_prefixes.is_empty()
                || self
                    .path_prefixes
                    .iter()
                    .any(|prefix| path_starts_with_rule(&path, prefix)))
            && (self.path_regex.is_none()
                || self.path_regex.as_ref().is_some_and(|regex| regex.is_match(&path)))
            && (self.query_keys.is_empty()
                || url.query_pairs().any(|(key, _)| {
                    let key = key.to_ascii_lowercase();
                    self.query_keys.iter().any(|candidate| key == *candidate)
                }))
            && (self.query_values.is_empty()
                || url.query_pairs().any(|(key, value)| {
                    let key = key.to_ascii_lowercase();
                    let value = value.to_ascii_lowercase();
                    self.query_values
                        .iter()
                        .any(|(candidate_key, candidate_value)| {
                            key == *candidate_key && value == *candidate_value
                        })
                }))
            && (self.query_value_prefixes.is_empty()
                || url.query_pairs().any(|(key, value)| {
                    let key = key.to_ascii_lowercase();
                    let value = value.to_ascii_lowercase();
                    self.query_value_prefixes
                        .iter()
                        .any(|(candidate_key, prefix)| {
                            key == *candidate_key && value.starts_with(prefix)
                        })
                }))
    }
}

#[derive(Debug, Deserialize)]
struct PlatformDefinitionFile {
    id: String,
    #[serde(default)]
    priority: i32,
    preset: Option<String>,
    #[serde(default, rename = "match")]
    match_rule: MatchRuleFile,
}

#[derive(Debug, Deserialize)]
struct PlatformPresetFile {
    id: String,
    default_action: Option<PageAction>,
    #[serde(default)]
    render: RenderRuleFile,
    #[serde(default)]
    discover: DiscoveryRuleFile,
    #[serde(default)]
    rules: Vec<PageRuleFile>,
}

#[derive(Debug, Deserialize)]
struct SitePresetFile {
    id: String,
    #[serde(default)]
    priority: i32,
    extends: Option<String>,
    default_action: Option<PageAction>,
    #[serde(default, rename = "match")]
    match_rule: MatchRuleFile,
    #[serde(default)]
    render: RenderRuleFile,
    #[serde(default)]
    discover: DiscoveryRuleFile,
    #[serde(default)]
    rules: Vec<PageRuleFile>,
}

#[derive(Debug, Deserialize, Default)]
struct MatchRuleFile {
    #[serde(default)]
    hosts: Vec<String>,
    #[serde(default)]
    host_suffixes: Vec<String>,
    #[serde(default)]
    path_prefixes: Vec<String>,
    #[serde(default)]
    path_regexes: Vec<String>,
    #[serde(default)]
    html_markers: Vec<String>,
    #[serde(default)]
    html_regexes: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RenderRuleFile {
    prefer_js: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct DiscoveryRuleFile {
    #[serde(default)]
    sources: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PageRuleFile {
    name: String,
    action: PageAction,
    reason: Option<String>,
    #[serde(default)]
    hosts: Vec<String>,
    #[serde(default)]
    host_suffixes: Vec<String>,
    #[serde(default)]
    path_exacts: Vec<String>,
    #[serde(default)]
    path_prefixes: Vec<String>,
    path_regex: Option<String>,
    #[serde(default)]
    query_keys: Vec<String>,
    #[serde(default)]
    query_values: Vec<String>,
    #[serde(default)]
    query_value_prefixes: Vec<String>,
}

#[derive(Debug, Clone)]
struct RuleSource {
    name: String,
    contents: String,
}

fn load_platform_identifiers(directory: &Path) -> Result<Vec<PlatformIdentifier>> {
    load_platform_identifiers_from_sources(read_rule_sources(directory)?)
}

fn load_platform_identifiers_from_bundle(files: &[SiteRuleFile]) -> Result<Vec<PlatformIdentifier>> {
    load_platform_identifiers_from_sources(rule_sources_from_bundle("platforms", files))
}

fn load_platform_identifiers_from_sources(sources: Vec<RuleSource>) -> Result<Vec<PlatformIdentifier>> {
    let mut identifiers = Vec::new();
    for source in sources {
        let file: PlatformDefinitionFile =
            toml::from_str(&source.contents).with_context(|| format!("parse {}", source.name))?;
        identifiers.push(PlatformIdentifier {
            preset: file.preset.clone().unwrap_or_else(|| file.id.clone()),
            id: file.id,
            priority: file.priority,
            matcher: compile_match_rule(&source.name, file.match_rule)?,
        });
    }
    Ok(identifiers)
}

fn load_platform_presets(directory: &Path) -> Result<BTreeMap<String, EffectivePreset>> {
    load_platform_presets_from_sources(read_rule_sources(directory)?)
}

fn load_platform_presets_from_bundle(
    files: &[SiteRuleFile],
) -> Result<BTreeMap<String, EffectivePreset>> {
    load_platform_presets_from_sources(rule_sources_from_bundle("platform-presets", files))
}

fn load_platform_presets_from_sources(
    sources: Vec<RuleSource>,
) -> Result<BTreeMap<String, EffectivePreset>> {
    let mut presets = BTreeMap::new();
    for source in sources {
        let file: PlatformPresetFile =
            toml::from_str(&source.contents).with_context(|| format!("parse {}", source.name))?;
        let rules = file
            .rules
            .into_iter()
            .map(|rule| compile_page_rule(&source.name, rule))
            .collect::<Result<Vec<_>>>()?;
        presets.insert(
            file.id.clone(),
            EffectivePreset {
                id: file.id,
                prefer_js: file.render.prefer_js.unwrap_or(false),
                default_action: file.default_action.unwrap_or_default(),
                discovery_sources: file.discover.sources,
                rules,
            },
        );
    }
    Ok(presets)
}

fn load_site_presets(directory: &Path) -> Result<Vec<SitePreset>> {
    load_site_presets_from_sources(read_rule_sources(directory)?)
}

fn load_site_presets_from_bundle(files: &[SiteRuleFile]) -> Result<Vec<SitePreset>> {
    load_site_presets_from_sources(rule_sources_from_bundle("sites", files))
}

fn load_site_presets_from_sources(sources: Vec<RuleSource>) -> Result<Vec<SitePreset>> {
    let mut presets = Vec::new();
    for source in sources {
        let file: SitePresetFile =
            toml::from_str(&source.contents).with_context(|| format!("parse {}", source.name))?;
        let rules = file
            .rules
            .into_iter()
            .map(|rule| compile_page_rule(&source.name, rule))
            .collect::<Result<Vec<_>>>()?;
        presets.push(SitePreset {
            id: file.id.clone(),
            priority: file.priority,
            extends: file.extends,
            matcher: compile_match_rule(&source.name, file.match_rule)?,
            preset: PresetOverride {
                id: file.id,
                prefer_js: file.render.prefer_js,
                default_action: file.default_action,
                discovery_sources: file.discover.sources,
                rules,
            },
        });
    }
    Ok(presets)
}

fn read_rule_sources(directory: &Path) -> Result<Vec<RuleSource>> {
    let mut sources = Vec::new();
    for path in toml_files(directory)? {
        sources.push(RuleSource {
            name: path.display().to_string(),
            contents: fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?,
        });
    }
    Ok(sources)
}

fn rule_sources_from_bundle(prefix: &str, files: &[SiteRuleFile]) -> Vec<RuleSource> {
    let mut sources: Vec<RuleSource> = files
        .iter()
        .map(|file| RuleSource {
            name: format!("{prefix}/{}", file.name),
            contents: file.content.clone(),
        })
        .collect();
    sources.sort_by(|left, right| left.name.cmp(&right.name));
    sources
}

fn toml_files(directory: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)
        .with_context(|| format!("read rules directory {}", directory.display()))?
    {
        let entry = entry.with_context(|| format!("read entry in {}", directory.display()))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn compile_match_rule(source: &str, rule: MatchRuleFile) -> Result<CompiledMatchRule> {
    Ok(CompiledMatchRule {
        hosts: normalize_hosts(rule.hosts),
        host_suffixes: normalize_hosts(rule.host_suffixes),
        path_prefixes: normalize_paths(rule.path_prefixes),
        path_regexes: rule
            .path_regexes
            .into_iter()
            .map(|pattern| {
                Regex::new(&format!("(?i){pattern}"))
                    .with_context(|| format!("compile path regex {pattern} in {source}"))
            })
            .collect::<Result<Vec<_>>>()?,
        html_markers: rule
            .html_markers
            .into_iter()
            .map(|marker| marker.to_ascii_lowercase())
            .collect(),
        html_regexes: rule
            .html_regexes
            .into_iter()
            .map(|pattern| {
                Regex::new(&format!("(?i){pattern}"))
                    .with_context(|| format!("compile html regex {pattern} in {source}"))
            })
            .collect::<Result<Vec<_>>>()?,
    })
}

fn compile_page_rule(source: &str, rule: PageRuleFile) -> Result<CompiledPageRule> {
    let query_values = rule
        .query_values
        .into_iter()
        .map(|pair| {
            let (key, value) = pair.split_once('=').ok_or_else(|| {
                anyhow!(
                    "invalid query_values entry `{pair}` in {source} (expected key=value)"
                )
            })?;
            Ok((key.trim().to_ascii_lowercase(), value.trim().to_ascii_lowercase()))
        })
        .collect::<Result<Vec<_>>>()?;
    let query_value_prefixes = rule
        .query_value_prefixes
        .into_iter()
        .map(|pair| {
            let (key, value) = pair.split_once('=').ok_or_else(|| {
                anyhow!(
                    "invalid query_value_prefixes entry `{pair}` in {source} (expected key=value)"
                )
            })?;
            Ok((key.trim().to_ascii_lowercase(), value.trim().to_ascii_lowercase()))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(CompiledPageRule {
        action: rule.action,
        reason: rule.reason.or_else(|| {
            (rule.action == PageAction::Deny)
                .then(|| format!("page skipped by rule {}", rule.name))
        }),
        hosts: normalize_hosts(rule.hosts),
        host_suffixes: normalize_hosts(rule.host_suffixes),
        path_exacts: normalize_paths(rule.path_exacts),
        path_prefixes: normalize_paths(rule.path_prefixes),
        path_regex: rule
            .path_regex
            .map(|pattern| {
                Regex::new(&format!("(?i){pattern}"))
                    .with_context(|| format!("compile path regex {pattern} in {source}"))
            })
            .transpose()?,
        query_keys: rule
            .query_keys
            .into_iter()
            .map(|key| key.to_ascii_lowercase())
            .collect(),
        query_values,
        query_value_prefixes,
    })
}

fn validate_registry(
    platform_identifiers: &[PlatformIdentifier],
    platform_presets: &BTreeMap<String, EffectivePreset>,
    site_presets: &[SitePreset],
) -> Result<()> {
    for identifier in platform_identifiers {
        if !platform_presets.contains_key(&identifier.preset) {
            return Err(anyhow!(
                "platform {} references missing preset {}",
                identifier.id,
                identifier.preset
            ));
        }
    }

    for site in site_presets {
        if let Some(base) = site.extends.as_deref() {
            if !platform_presets.contains_key(base) {
                return Err(anyhow!("site {} extends missing preset {}", site.id, base));
            }
        }
    }

    Ok(())
}

fn normalize_hosts(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalize_paths(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn path_starts_with_rule(path: &str, rule: &str) -> bool {
    if rule.ends_with('/') {
        path.starts_with(rule)
    } else {
        path == rule || path.strip_prefix(rule).is_some_and(|rest| rest.starts_with('/'))
    }
}
