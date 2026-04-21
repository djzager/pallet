use super::util;
use super::{AgentAdapter, PlaceResult};
use crate::resource::RawResource;
use anyhow::Result;
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
        fs::create_dir_all(&opencode_dir)?;

        let (content, hashes) =
            util::build_concatenated_instructions(resources, crate::builtin::PALLET_SKILL);

        let instructions_path = opencode_dir.join("instructions.md");
        util::write_readonly(&instructions_path, &content)?;

        let placed = ".opencode/instructions.md".to_string();
        println!("    Concatenated instructions: {}", placed);

        Ok(PlaceResult {
            hashes,
            placed_paths: vec![placed],
        })
    }

    fn cleanup_placed(&self, workspace: &Path, placed_paths: &[String]) -> Result<()> {
        for rel_path in placed_paths {
            util::remove_placed(workspace, rel_path)?;
        }
        Ok(())
    }
}
