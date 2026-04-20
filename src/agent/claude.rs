use crate::resource::{RawResource, ResourceContent, ResourceKind};
use crate::store;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub struct PlaceResult {
    pub hashes: HashMap<String, String>,
    pub placed_paths: Vec<String>,
}

/// Detect if Claude Code is present
pub fn detect(workspace: &Path) -> bool {
    if workspace.join(".claude").is_dir() {
        return true;
    }

    std::process::Command::new("which")
        .arg("claude")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Place resources directly into .claude/ directories, returning hashes and placed paths
pub fn place(workspace: &Path, resources: &[RawResource]) -> Result<PlaceResult> {
    let claude_dir = workspace.join(".claude");

    // Ensure placement directories exist
    fs::create_dir_all(claude_dir.join("skills"))?;
    fs::create_dir_all(claude_dir.join("rules"))?;
    fs::create_dir_all(claude_dir.join("agents"))?;

    // Place built-in pallet skill
    place_builtin_skill(&claude_dir)?;

    let mut hashes = HashMap::new();
    let mut placed_paths = Vec::new();

    for resource in resources {
        match resource.kind {
            ResourceKind::Skill => {
                place_skill(&claude_dir, resource, &mut hashes, &mut placed_paths)?;
            }
            ResourceKind::Rule => {
                place_single_file(
                    &claude_dir, resource, "rules",
                    &mut hashes, &mut placed_paths,
                )?;
            }
            ResourceKind::Agent => {
                place_single_file(
                    &claude_dir, resource, "agents",
                    &mut hashes, &mut placed_paths,
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

/// Write a skill directory directly to .claude/skills/{name}/
fn place_skill(
    claude_dir: &Path,
    resource: &RawResource,
    hashes: &mut HashMap<String, String>,
    placed_paths: &mut Vec<String>,
) -> Result<()> {
    let skill_dir = claude_dir.join("skills").join(&resource.name);

    // Remove existing directory to ensure clean state
    if skill_dir.exists() {
        make_tree_writable(&skill_dir)?;
        fs::remove_dir_all(&skill_dir)?;
    }
    fs::create_dir_all(&skill_dir)?;

    if let ResourceContent::Directory { files } = &resource.content {
        for (relative_path, content) in files {
            let file_path = skill_dir.join(relative_path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&file_path, content)?;
            set_readonly(&file_path)?;

            let hash = store::sha256_hex(content);
            hashes.insert(
                format!("skills/{}/{}", resource.name, relative_path),
                hash,
            );
        }
    }

    let placed = format!(
        ".claude/skills/{}",
        resource.name
    );
    println!(
        "    Skill '{}': {}",
        resource.name, placed
    );
    placed_paths.push(placed);

    Ok(())
}

/// Write a single-file resource (rule or agent) directly to .claude/{subdir}/{NN}-{source}-{name}.md
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

    let placed_name = format!(
        "{:02}-{}-{}",
        resource.source_index, resource.source_name, filename
    );
    let file_path = claude_dir.join(claude_subdir).join(&placed_name);

    // Make writable if exists (to allow overwrite of 0444 file)
    if file_path.exists() {
        let mut perms = fs::metadata(&file_path)?.permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&file_path, perms)?;
    }

    if let ResourceContent::SingleFile { content, .. } = &resource.content {
        fs::write(&file_path, content)?;
        set_readonly(&file_path)?;

        let hash = store::sha256_hex(content);
        hashes.insert(
            format!("{}/{}", claude_subdir, placed_name),
            hash,
        );
    }

    let placed = format!(".claude/{}/{}", claude_subdir, placed_name);
    println!(
        "    {} '{}': {}",
        resource.kind, resource.name, placed
    );
    placed_paths.push(placed);

    Ok(())
}

/// Place the built-in pallet self-awareness skill
fn place_builtin_skill(claude_dir: &Path) -> Result<()> {
    let skill_dir = claude_dir.join("skills").join("pallet");
    fs::create_dir_all(&skill_dir)?;

    let skill_path = skill_dir.join("SKILL.md");

    // Make writable if exists
    if skill_path.exists() {
        let mut perms = fs::metadata(&skill_path)?.permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&skill_path, perms)?;
    }

    fs::write(&skill_path, crate::builtin::PALLET_SKILL)?;
    set_readonly(&skill_path)?;

    println!("    Built-in skill 'pallet': .claude/skills/pallet/SKILL.md");

    Ok(())
}

fn set_readonly(path: &Path) -> Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o444);
    fs::set_permissions(path, perms)?;
    Ok(())
}

fn make_tree_writable(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            make_tree_writable(&path)?;
        } else {
            let mut perms = fs::metadata(&path)?.permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&path, perms)?;
        }
    }
    Ok(())
}

/// Clean up previously-placed resources from a prior sync
pub fn cleanup_placed(workspace: &Path, placed_paths: &[String]) -> Result<()> {
    for rel_path in placed_paths {
        let full_path = workspace.join(rel_path);
        if full_path.is_dir() {
            make_tree_writable(&full_path)?;
            fs::remove_dir_all(&full_path)?;
            println!("    Removed directory: {}", rel_path);
        } else if full_path.is_file() {
            let mut perms = fs::metadata(&full_path)?.permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&full_path, perms)?;
            fs::remove_file(&full_path)?;
            println!("    Removed file: {}", rel_path);
        }
    }
    Ok(())
}
