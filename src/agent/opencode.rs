use super::util;
use super::{AgentAdapter, PlaceResult};
use crate::resource::{RawResource, ResourceKind};
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct OpenCodeAdapter;

impl AgentAdapter for OpenCodeAdapter {
    fn name(&self) -> &str {
        "opencode"
    }

    fn display_name(&self) -> &str {
        "OpenCode"
    }

    fn detect(&self, workspace: &Path) -> bool {
        util::detect_by_dir_or_binary(workspace, ".opencode", "opencode")
    }

    fn place(&self, workspace: &Path, resources: &[RawResource]) -> Result<PlaceResult> {
        let opencode_dir = workspace.join(".opencode");
        let memories_dir = opencode_dir.join("memories");
        let skills_dir = opencode_dir.join("skills");
        fs::create_dir_all(&memories_dir)?;
        fs::create_dir_all(&skills_dir)?;

        // Place built-in pallet skill
        util::place_builtin_skill(&skills_dir, ".opencode")?;

        let mut hashes = HashMap::new();
        let mut placed_paths = Vec::new();

        for resource in resources {
            match resource.kind {
                ResourceKind::Rule | ResourceKind::Agent => {
                    // Individual plain markdown files in .opencode/memories/
                    util::place_rule_as_plain_md(
                        &memories_dir,
                        resource,
                        ".opencode/memories",
                        &mut hashes,
                        &mut placed_paths,
                    )?;
                }
                ResourceKind::Skill => {
                    // Agent Skills directories in .opencode/skills/
                    util::place_skill_directory(
                        &skills_dir,
                        resource,
                        ".opencode",
                        &mut hashes,
                        &mut placed_paths,
                    )?;
                }
                ResourceKind::Prompt | ResourceKind::Profile => {}
            }
        }

        Ok(PlaceResult {
            hashes,
            placed_paths,
        })
    }

    fn cleanup_placed(&self, workspace: &Path, placed_paths: &[String]) -> Result<()> {
        for rel_path in placed_paths {
            util::remove_placed(workspace, rel_path)?;
        }
        Ok(())
    }

    fn always_loaded_kinds(&self) -> Vec<ResourceKind> {
        vec![ResourceKind::Rule, ResourceKind::Agent]
    }
}
