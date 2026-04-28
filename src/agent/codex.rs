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
        let memories_dir = codex_dir.join("memories");
        let skills_dir = codex_dir.join("skills");
        let agents_dir = codex_dir.join("agents");
        fs::create_dir_all(&memories_dir)?;
        fs::create_dir_all(&skills_dir)?;
        fs::create_dir_all(&agents_dir)?;

        // Place built-in pallet skill
        util::place_builtin_skill(&skills_dir, ".codex")?;

        let mut hashes = HashMap::new();
        let mut placed_paths = Vec::new();

        for resource in resources {
            match resource.kind {
                ResourceKind::Rule => {
                    // Individual plain markdown files in .codex/memories/
                    util::place_rule_as_plain_md(
                        &memories_dir,
                        resource,
                        ".codex/memories",
                        &mut hashes,
                        &mut placed_paths,
                    )?;
                }
                ResourceKind::Agent => {
                    // Agent definitions as individual files in .codex/agents/
                    place_agent_file(&agents_dir, resource, &mut hashes, &mut placed_paths)?;
                }
                ResourceKind::Skill => {
                    // Agent Skills directories in .codex/skills/
                    util::place_skill_directory(
                        &skills_dir,
                        resource,
                        ".codex",
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

/// Place an agent definition as an individual file in .codex/agents/
fn place_agent_file(
    agents_dir: &Path,
    resource: &RawResource,
    hashes: &mut HashMap<String, String>,
    placed_paths: &mut Vec<String>,
) -> Result<()> {
    let filename = match &resource.content {
        ResourceContent::SingleFile { filename, .. } => filename.clone(),
        _ => format!("{}.md", resource.name),
    };

    let placed_name =
        util::prefixed_filename(resource.source_index, &resource.source_name, &filename);
    let file_path = agents_dir.join(&placed_name);

    if let ResourceContent::SingleFile { content, .. } = &resource.content {
        util::write_readonly(&file_path, content)?;

        let hash = store::sha256_hex(content);
        hashes.insert(format!("agents/{}", placed_name), hash);
    }

    let placed = format!(".codex/agents/{}", placed_name);
    println!("    Agent '{}': {}", resource.name, placed);
    placed_paths.push(placed);

    Ok(())
}
