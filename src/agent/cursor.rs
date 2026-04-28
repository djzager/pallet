use super::util;
use super::{AgentAdapter, PlaceResult};
use crate::resource::{RawResource, ResourceContent, ResourceKind};
use crate::store;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct CursorAdapter;

impl AgentAdapter for CursorAdapter {
    fn name(&self) -> &str {
        "cursor"
    }

    fn display_name(&self) -> &str {
        "Cursor"
    }

    fn detect(&self, workspace: &Path) -> bool {
        util::detect_by_dir_or_binary(workspace, ".cursor", "cursor")
    }

    fn place(&self, workspace: &Path, resources: &[RawResource]) -> Result<PlaceResult> {
        let cursor_dir = workspace.join(".cursor");
        let rules_dir = cursor_dir.join("rules");
        let skills_dir = cursor_dir.join("skills");
        fs::create_dir_all(&rules_dir)?;
        fs::create_dir_all(&skills_dir)?;

        // Place built-in pallet skill
        util::place_builtin_skill(&skills_dir, ".cursor")?;

        let mut hashes = HashMap::new();
        let mut placed_paths = Vec::new();

        for resource in resources {
            match resource.kind {
                ResourceKind::Rule | ResourceKind::Agent => {
                    place_mdc_file(&rules_dir, resource, &mut hashes, &mut placed_paths)?;
                }
                ResourceKind::Skill => {
                    // Agent Skills directories in .cursor/skills/
                    util::place_skill_directory(
                        &skills_dir,
                        resource,
                        ".cursor",
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

    fn is_always_loaded(&self, resource: &RawResource) -> bool {
        if resource.globs.is_some() || resource.description.is_some() {
            return false; // Conditional — Cursor loads based on globs/description
        }
        self.always_loaded_kinds().contains(&resource.kind)
    }
}

/// Place a single-file resource as a .mdc file in .cursor/rules/ with proper frontmatter
fn place_mdc_file(
    rules_dir: &Path,
    resource: &RawResource,
    hashes: &mut HashMap<String, String>,
    placed_paths: &mut Vec<String>,
) -> Result<()> {
    let base_name = match &resource.content {
        ResourceContent::SingleFile { filename, .. } => {
            // Strip .md extension, we'll add .mdc
            filename.strip_suffix(".md").unwrap_or(filename).to_string()
        }
        _ => resource.name.clone(),
    };

    let placed_name = format!(
        "{}.mdc",
        util::prefixed_filename(resource.source_index, &resource.source_name, &base_name)
    );
    let file_path = rules_dir.join(&placed_name);

    if let ResourceContent::SingleFile { content, .. } = &resource.content {
        let text = String::from_utf8_lossy(content);
        let body = util::strip_frontmatter(&text);

        // Generate .mdc frontmatter based on conditional loading metadata
        let frontmatter = if let Some(ref globs) = resource.globs {
            let globs_yaml: Vec<String> = globs.iter().map(|g| format!("  - \"{}\"", g)).collect();
            let desc = resource.description.as_deref().unwrap_or(&resource.name);
            format!(
                "---\nalwaysApply: false\ndescription: \"{}\"\nglobs:\n{}\n---",
                desc,
                globs_yaml.join("\n")
            )
        } else if let Some(ref desc) = resource.description {
            format!("---\nalwaysApply: false\ndescription: \"{}\"\n---", desc)
        } else {
            "---\nalwaysApply: true\n---".to_string()
        };

        let mdc_content = format!("{}\n\n{}", frontmatter, body.trim());
        let output = mdc_content.as_bytes();

        util::write_readonly(&file_path, output)?;

        let hash = store::sha256_hex(output);
        hashes.insert(format!("rules/{}", placed_name), hash);
    }

    let placed = format!(".cursor/rules/{}", placed_name);
    println!("    {} '{}': {}", resource.kind, resource.name, placed);
    placed_paths.push(placed);

    Ok(())
}
