use crate::resource::ResourceKind;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const HOME_DIR: &str = ".pallet";
pub const CONFIG_FILE: &str = "pallet.yaml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub hub: Option<HubConfig>,
    pub sources: Vec<SourceConfig>,
    pub agents: AgentsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubConfig {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hub_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub source_type: SourceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<PathEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,
}

/// A path entry in a source config, supporting both simple strings and annotated paths.
///
/// Simple: `"skills"` — just a path, no kind hint
/// Annotated: `{ path: "agents", kind: "rule" }` — path with explicit resource kind
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PathEntry {
    Simple(String),
    Annotated {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        kind: Option<ResourceKind>,
        #[serde(skip_serializing_if = "Option::is_none")]
        globs: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
}

impl PathEntry {
    pub fn path(&self) -> &str {
        match self {
            PathEntry::Simple(s) => s,
            PathEntry::Annotated { path, .. } => path,
        }
    }

    pub fn kind_hint(&self) -> Option<&ResourceKind> {
        match self {
            PathEntry::Simple(_) => None,
            PathEntry::Annotated { kind, .. } => kind.as_ref(),
        }
    }

    pub fn globs(&self) -> Option<&Vec<String>> {
        match self {
            PathEntry::Simple(_) => None,
            PathEntry::Annotated { globs, .. } => globs.as_ref(),
        }
    }

    pub fn description(&self) -> Option<&str> {
        match self {
            PathEntry::Simple(_) => None,
            PathEntry::Annotated { description, .. } => description.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Git,
    Hub,
    Local,
}

impl SourceConfig {
    pub fn source_type_str(&self) -> &str {
        match self.source_type {
            SourceType::Git => "git",
            SourceType::Hub => "hub",
            SourceType::Local => "local",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    pub auto_detect: bool,
}

// --- Project-root config (pallet.yaml) ---

/// Returns workspace/pallet.yaml path
pub fn config_path(workspace: &Path) -> PathBuf {
    workspace.join(CONFIG_FILE)
}

pub fn load_config(workspace: &Path) -> Result<Config> {
    let path = config_path(workspace);
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config at {}", path.display()))?;
    let config: Config =
        serde_yaml::from_str(&content).context("Failed to parse pallet.yaml")?;
    Ok(config)
}

pub fn save_config(workspace: &Path, config: &Config) -> Result<()> {
    let path = config_path(workspace);
    let yaml = serde_yaml::to_string(config)?;
    fs::write(&path, yaml)?;
    Ok(())
}

// --- Home directory (~/.pallet/) for credentials and cache ---

/// Returns ~/.pallet/ path
fn home_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home).join(HOME_DIR))
}

/// Returns ~/.pallet/cache/ path
pub fn cache_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join("cache"))
}

/// Returns ~/.pallet/credentials.yaml path
fn credentials_path() -> Result<PathBuf> {
    Ok(home_dir()?.join("credentials.yaml"))
}

pub fn load_credentials() -> Result<Credentials> {
    let path = credentials_path()?;
    let content = fs::read_to_string(&path)
        .with_context(|| format!("No credentials found at {}. Run `pallet auth` first.", path.display()))?;
    let creds: Credentials =
        serde_yaml::from_str(&content).context("Failed to parse credentials.yaml")?;
    Ok(creds)
}

pub fn save_credentials(creds: &Credentials) -> Result<()> {
    let dir = home_dir()?;
    fs::create_dir_all(&dir)?;
    let path = credentials_path()?;
    let yaml = serde_yaml::to_string(creds)?;
    fs::write(&path, yaml)?;

    // Restrict permissions on credentials file
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(&path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(&path, perms)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_entry_simple_deserialization() {
        let yaml = r#"- skills
- rules"#;
        let entries: Vec<PathEntry> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path(), "skills");
        assert!(entries[0].kind_hint().is_none());
    }

    #[test]
    fn test_path_entry_annotated_deserialization() {
        let yaml = r#"- path: agents
  kind: rule"#;
        let entries: Vec<PathEntry> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path(), "agents");
        assert_eq!(entries[0].kind_hint(), Some(&ResourceKind::Rule));
    }

    #[test]
    fn test_path_entry_mixed_deserialization() {
        let yaml = r#"- skills
- path: agents
  kind: agent"#;
        let entries: Vec<PathEntry> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path(), "skills");
        assert!(entries[0].kind_hint().is_none());
        assert_eq!(entries[1].path(), "agents");
        assert_eq!(entries[1].kind_hint(), Some(&ResourceKind::Agent));
    }
}
