pub mod claude;
pub mod codex;
pub mod cursor;
pub mod goose;
pub mod opencode;
pub(crate) mod util;

use crate::resource::{RawResource, ResourceKind};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// Result of placing resources for a single agent
pub struct PlaceResult {
    /// Map of logical path -> content hash (e.g., "rules/00-src-name.md" -> "sha256:...")
    pub hashes: HashMap<String, String>,
    /// Paths placed relative to workspace root (e.g., ".claude/rules/00-src-name.md")
    pub placed_paths: Vec<String>,
}

/// Default context budget in bytes (~30K tokens * 4 bytes/token)
pub const DEFAULT_CONTEXT_BUDGET_BYTES: usize = 120_000;

/// Trait that every agent adapter must implement
pub trait AgentAdapter {
    /// Machine-readable identifier (e.g., "claude", "cursor", "goose")
    fn name(&self) -> &str;

    /// Human-readable label for display (e.g., "Claude Code", "Cursor")
    fn display_name(&self) -> &str;

    /// Detect whether this agent is configured/present in the workspace
    fn detect(&self, workspace: &Path) -> bool;

    /// Place all resources into the agent's expected directories.
    fn place(&self, workspace: &Path, resources: &[RawResource]) -> Result<PlaceResult>;

    /// Remove previously-placed resources tracked by the given relative paths.
    fn cleanup_placed(&self, workspace: &Path, placed_paths: &[String]) -> Result<()>;

    /// Resource kinds that are always loaded into the agent's context at startup.
    /// Used for context budget estimation.
    fn always_loaded_kinds(&self) -> Vec<ResourceKind> {
        vec![ResourceKind::Rule]
    }

    /// Whether a specific resource is always loaded (counts toward budget).
    /// Default: true if resource kind is in always_loaded_kinds().
    /// Claude/Cursor override: false if resource has globs or description (conditional loading).
    fn is_always_loaded(&self, resource: &RawResource) -> bool {
        self.always_loaded_kinds().contains(&resource.kind)
    }

    /// Maximum bytes of always-loaded content before warning.
    fn context_budget_bytes(&self) -> usize {
        DEFAULT_CONTEXT_BUDGET_BYTES
    }
}

/// Returns all known agent adapters
pub fn all_adapters() -> Vec<Box<dyn AgentAdapter>> {
    vec![
        Box::new(claude::ClaudeAdapter),
        Box::new(cursor::CursorAdapter),
        Box::new(goose::GooseAdapter),
        Box::new(opencode::OpenCodeAdapter),
        Box::new(codex::CodexAdapter),
    ]
}
