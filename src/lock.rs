use crate::agent::PlaceResult;
use crate::config::SourceConfig;
use crate::resource::RawResource;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LockFile {
    /// Hash of the config file content that produced this lock
    pub config_hash: String,
    /// Resolved source references
    pub sources: Vec<LockedSource>,
    /// Per-resource manifest
    pub resources: Vec<LockedResource>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LockedSource {
    pub name: String,
    #[serde(rename = "type")]
    pub source_type: String,
    /// Resolved git SHA or hub profile version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LockedResource {
    pub kind: String,
    pub name: String,
    pub source: String,
    pub source_index: usize,
    pub governance: String,
    pub content_hash: String,
    /// Paths placed per agent, keyed by agent name
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub placed: HashMap<String, Vec<String>>,
}

/// Build a lock file from sync results
pub fn build_lock(
    fetch_results: &[(&SourceConfig, Option<String>)],
    resources: &[RawResource],
    agent_results: &HashMap<String, PlaceResult>,
    config_hash: &str,
) -> LockFile {
    let sources = fetch_results
        .iter()
        .map(|(cfg, resolved_ref)| LockedSource {
            name: cfg.name.clone(),
            source_type: cfg.source_type_str().to_string(),
            resolved_ref: resolved_ref.clone(),
            url: cfg.url.clone(),
        })
        .collect();

    let mut locked_resources: Vec<LockedResource> = resources
        .iter()
        .map(|r| {
            // Find the best matching hash for this resource across all agents
            let kind_dir = r.kind.dir_name();
            let content_hash = agent_results
                .values()
                .flat_map(|pr| pr.hashes.iter())
                .find(|(k, _)| k.starts_with(&format!("{}/{}", kind_dir, r.name)))
                .map(|(_, v)| v.clone())
                .unwrap_or_default();

            // Find placed paths per agent for this resource
            let mut placed: HashMap<String, Vec<String>> = HashMap::new();
            for (agent_name, pr) in agent_results {
                let resource_placed: Vec<String> = pr
                    .placed_paths
                    .iter()
                    .filter(|p| p.contains(&r.name))
                    .cloned()
                    .collect();
                if !resource_placed.is_empty() {
                    placed.insert(agent_name.clone(), resource_placed);
                }
            }

            LockedResource {
                kind: r.kind.to_string(),
                name: r.name.clone(),
                source: r.source_name.clone(),
                source_index: r.source_index,
                governance: r.governance.clone(),
                content_hash,
                placed,
            }
        })
        .collect();

    // Sort resources deterministically by (kind, name) for merge-friendly diffs
    locked_resources.sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));

    LockFile {
        config_hash: config_hash.to_string(),
        sources,
        resources: locked_resources,
    }
}

/// Write lock file to pallet.lock in the workspace root
pub fn save_lock(workspace: &Path, lock: &LockFile) -> Result<()> {
    let lock_path = workspace.join("pallet.lock");
    let yaml = serde_yaml::to_string(lock)?;
    fs::write(&lock_path, yaml)
        .with_context(|| format!("Failed to write lock file at {}", lock_path.display()))?;
    Ok(())
}

/// Read lock file from pallet.lock in the workspace root
pub fn load_lock(workspace: &Path) -> Result<LockFile> {
    let lock_path = workspace.join("pallet.lock");
    let content = fs::read_to_string(&lock_path)
        .with_context(|| format!("No lock file found at {}. Run `pallet sync` first.", lock_path.display()))?;
    let lock: LockFile =
        serde_yaml::from_str(&content).context("Failed to parse pallet.lock")?;
    Ok(lock)
}

/// Collect all placed paths from a lock file, keyed by agent name
pub fn all_placed_paths(lock: &LockFile) -> HashMap<String, Vec<String>> {
    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    for r in &lock.resources {
        for (agent, paths) in &r.placed {
            result
                .entry(agent.clone())
                .or_default()
                .extend(paths.iter().cloned());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_file_roundtrip() {
        let mut placed = HashMap::new();
        placed.insert(
            "claude".to_string(),
            vec![".claude/rules/00-test-source-test-rule.md".to_string()],
        );

        let lock = LockFile {
            config_hash: "sha256:abc123".to_string(),
            sources: vec![LockedSource {
                name: "test-source".to_string(),
                source_type: "git".to_string(),
                resolved_ref: Some("abc123def456".to_string()),
                url: Some("https://github.com/test/repo".to_string()),
            }],
            resources: vec![LockedResource {
                kind: "rule".to_string(),
                name: "test-rule".to_string(),
                source: "test-source".to_string(),
                source_index: 0,
                governance: "federated".to_string(),
                content_hash: "sha256:deadbeef".to_string(),
                placed,
            }],
        };

        let yaml = serde_yaml::to_string(&lock).unwrap();
        let parsed: LockFile = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed.config_hash, lock.config_hash);
        assert_eq!(parsed.sources.len(), 1);
        assert_eq!(parsed.sources[0].name, "test-source");
        assert_eq!(parsed.resources.len(), 1);
        assert_eq!(parsed.resources[0].name, "test-rule");
        let claude_paths = parsed.resources[0].placed.get("claude").unwrap();
        assert_eq!(claude_paths.len(), 1);
        assert_eq!(
            claude_paths[0],
            ".claude/rules/00-test-source-test-rule.md"
        );
    }
}
