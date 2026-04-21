use super::util;
use super::{AgentAdapter, PlaceResult};
use crate::resource::{RawResource, ResourceContent, ResourceKind};
use crate::store;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct CodexAdapter;

impl AgentAdapter for CodexAdapter {
    fn name(&self) -> &str {
        "codex"
    }

    fn display_name(&self) -> &str {
        "OpenAI Codex"
    }

    fn detect(&self, workspace: &Path) -> bool {
        util::detect_by_dir_or_binary(workspace, ".codex", "codex")
    }

    fn place(&self, workspace: &Path, resources: &[RawResource]) -> Result<PlaceResult> {
        let codex_dir = workspace.join(".codex");

        let mut hashes = HashMap::new();
        let mut placed_paths = Vec::new();

        // Separate agent resources (individual files) from rules/skills (concatenated)
        let mut concat_resources = Vec::new();
        let mut agent_resources = Vec::new();

        for resource in resources {
            match resource.kind {
                ResourceKind::Rule | ResourceKind::Skill => {
                    concat_resources.push(resource);
                }
                ResourceKind::Agent => {
                    agent_resources.push(resource);
                }
                ResourceKind::Prompt | ResourceKind::Profile => {}
            }
        }

        // Place concatenated instructions as codex.md at workspace root
        if !concat_resources.is_empty() || !agent_resources.is_empty() {
            let (content, concat_hashes) = util::build_concatenated_instructions(
                // Pass all resources — build_concatenated_instructions skips prompts/profiles
                resources,
                crate::builtin::PALLET_SKILL,
            );

            let codex_md_path = workspace.join("codex.md");
            util::write_readonly(&codex_md_path, &content)?;
            hashes.extend(concat_hashes);

            let placed = "codex.md".to_string();
            println!("    Concatenated instructions: {}", placed);
            placed_paths.push(placed);
        }

        // Place agent resources as individual files in .codex/agents/
        if !agent_resources.is_empty() {
            let agents_dir = codex_dir.join("agents");
            fs::create_dir_all(&agents_dir)?;

            for resource in &agent_resources {
                let filename = match &resource.content {
                    ResourceContent::SingleFile { filename, .. } => filename.clone(),
                    _ => format!("{}.md", resource.name),
                };

                let placed_name = util::prefixed_filename(
                    resource.source_index,
                    &resource.source_name,
                    &filename,
                );
                let file_path = agents_dir.join(&placed_name);

                if let ResourceContent::SingleFile { content, .. } = &resource.content {
                    util::write_readonly(&file_path, content)?;

                    let hash = store::sha256_hex(content);
                    hashes.insert(format!("agents/{}", placed_name), hash);
                }

                let placed = format!(".codex/agents/{}", placed_name);
                println!("    Agent '{}': {}", resource.name, placed);
                placed_paths.push(placed);
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
}
