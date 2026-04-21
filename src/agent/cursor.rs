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
        let rules_dir = workspace.join(".cursor").join("rules");
        fs::create_dir_all(&rules_dir)?;

        // Place built-in pallet skill as a rule
        place_builtin_rule(&rules_dir)?;

        let mut hashes = HashMap::new();
        let mut placed_paths = Vec::new();

        for resource in resources {
            match resource.kind {
                ResourceKind::Rule | ResourceKind::Agent => {
                    place_mdc_file(
                        &rules_dir,
                        resource,
                        &mut hashes,
                        &mut placed_paths,
                    )?;
                }
                ResourceKind::Skill => {
                    place_skill_as_mdc(
                        &rules_dir,
                        resource,
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
}

/// Place a single-file resource as a .mdc file in .cursor/rules/
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
        util::write_readonly(&file_path, content)?;

        let hash = store::sha256_hex(content);
        hashes.insert(format!("rules/{}", placed_name), hash);
    }

    let placed = format!(".cursor/rules/{}", placed_name);
    println!("    {} '{}': {}", resource.kind, resource.name, placed);
    placed_paths.push(placed);

    Ok(())
}

/// Place a skill (Directory resource) as a single .mdc file, extracting SKILL.md content
fn place_skill_as_mdc(
    rules_dir: &Path,
    resource: &RawResource,
    hashes: &mut HashMap<String, String>,
    placed_paths: &mut Vec<String>,
) -> Result<()> {
    let content = match util::extract_primary_content(resource) {
        Some(c) => c,
        None => return Ok(()),
    };

    let placed_name = format!(
        "{}.mdc",
        util::prefixed_filename(resource.source_index, &resource.source_name, &resource.name)
    );
    let file_path = rules_dir.join(&placed_name);

    util::write_readonly(&file_path, content)?;

    let hash = store::sha256_hex(content);
    hashes.insert(format!("rules/{}", placed_name), hash);

    let placed = format!(".cursor/rules/{}", placed_name);
    println!("    Skill '{}': {}", resource.name, placed);
    placed_paths.push(placed);

    Ok(())
}

/// Place the built-in pallet skill as a .mdc rule
fn place_builtin_rule(rules_dir: &Path) -> Result<()> {
    let file_path = rules_dir.join("pallet.mdc");
    util::write_readonly(&file_path, crate::builtin::PALLET_SKILL.as_bytes())?;
    println!("    Built-in skill 'pallet': .cursor/rules/pallet.mdc");
    Ok(())
}
