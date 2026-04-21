use super::util;
use super::{AgentAdapter, PlaceResult};
use crate::resource::RawResource;
use anyhow::Result;
use std::fs;
use std::path::Path;

pub struct GooseAdapter;

impl AgentAdapter for GooseAdapter {
    fn name(&self) -> &str {
        "goose"
    }

    fn display_name(&self) -> &str {
        "Goose"
    }

    fn detect(&self, workspace: &Path) -> bool {
        util::detect_by_dir_or_binary(workspace, ".goose", "goose")
    }

    fn place(&self, workspace: &Path, resources: &[RawResource]) -> Result<PlaceResult> {
        let goose_dir = workspace.join(".goose");
        fs::create_dir_all(&goose_dir)?;

        let (content, hashes) =
            util::build_concatenated_instructions(resources, crate::builtin::PALLET_SKILL);

        let instructions_path = goose_dir.join("instructions.md");
        util::write_readonly(&instructions_path, &content)?;

        let placed = ".goose/instructions.md".to_string();
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
