use super::util;
use super::{AgentAdapter, PlaceResult};
use crate::resource::{RawResource, ResourceContent, ResourceKind};
use crate::store;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct ClaudeAdapter;

impl AgentAdapter for ClaudeAdapter {
    fn name(&self) -> &str {
        "claude"
    }

    fn display_name(&self) -> &str {
        "Claude Code"
    }

    fn detect(&self, workspace: &Path) -> bool {
        util::detect_by_dir_or_binary(workspace, ".claude", "claude")
    }

    fn place(&self, workspace: &Path, resources: &[RawResource]) -> Result<PlaceResult> {
        let claude_dir = workspace.join(".claude");

        // Ensure placement directories exist
        let skills_dir = claude_dir.join("skills");
        fs::create_dir_all(&skills_dir)?;
        fs::create_dir_all(claude_dir.join("rules"))?;
        fs::create_dir_all(claude_dir.join("agents"))?;

        // Place built-in pallet skill
        util::place_builtin_skill(&skills_dir, ".claude")?;

        let mut hashes = HashMap::new();
        let mut placed_paths = Vec::new();

        for resource in resources {
            match resource.kind {
                ResourceKind::Skill => {
                    util::place_skill_directory(
                        &skills_dir,
                        resource,
                        ".claude",
                        &mut hashes,
                        &mut placed_paths,
                    )?;
                }
                ResourceKind::Rule => {
                    place_single_file(
                        &claude_dir,
                        resource,
                        "rules",
                        &mut hashes,
                        &mut placed_paths,
                    )?;
                }
                ResourceKind::Agent => {
                    place_single_file(
                        &claude_dir,
                        resource,
                        "agents",
                        &mut hashes,
                        &mut placed_paths,
                    )?;
                }
                ResourceKind::Prompt => {
                    println!(
                        "    Prompt '{}' fetched (no Claude Code placement defined)",
                        resource.name
                    );
                }
                ResourceKind::Profile => {
                    println!(
                        "    Profile '{}' fetched (not placed for Claude — used by kantra)",
                        resource.name
                    );
                }
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

    fn is_always_loaded(&self, resource: &RawResource) -> bool {
        if resource.globs.is_some() || resource.description.is_some() {
            return false; // Conditional — Claude loads based on paths: frontmatter
        }
        self.always_loaded_kinds().contains(&resource.kind)
    }
}

/// Write a single-file resource (rule or agent) directly to .claude/{subdir}/{NN}-{source}-{name}.md
/// For rules with globs, translates to Claude-native `paths:` frontmatter.
fn place_single_file(
    claude_dir: &Path,
    resource: &RawResource,
    claude_subdir: &str,
    hashes: &mut HashMap<String, String>,
    placed_paths: &mut Vec<String>,
) -> Result<()> {
    let filename = match &resource.content {
        ResourceContent::SingleFile { filename, .. } => filename.clone(),
        _ => format!("{}.md", resource.name),
    };

    let placed_name =
        util::prefixed_filename(resource.source_index, &resource.source_name, &filename);
    let file_path = claude_dir.join(claude_subdir).join(&placed_name);

    if let ResourceContent::SingleFile { content, .. } = &resource.content {
        let output = if let (Some(globs), ResourceKind::Rule) =
            (&resource.globs, &resource.kind)
        {
            // Translate globs to Claude-native paths: frontmatter
            let text = String::from_utf8_lossy(content);
            let body = util::strip_frontmatter(&text);
            let paths_yaml: Vec<String> = globs.iter().map(|g| format!("  - \"{}\"", g)).collect();
            let new_content = format!(
                "---\npaths:\n{}\n---\n\n{}",
                paths_yaml.join("\n"),
                body.trim()
            );
            new_content.into_bytes()
        } else {
            content.clone()
        };

        util::write_readonly(&file_path, &output)?;

        let hash = store::sha256_hex(&output);
        hashes.insert(format!("{}/{}", claude_subdir, placed_name), hash);
    }

    let placed = format!(".claude/{}/{}", claude_subdir, placed_name);
    println!("    {} '{}': {}", resource.kind, resource.name, placed);
    placed_paths.push(placed);

    Ok(())
}
