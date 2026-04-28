use crate::resource::{RawResource, ResourceContent};
use crate::store;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Set a file to read-only (0444)
pub fn set_readonly(path: &Path) -> Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o444);
    fs::set_permissions(path, perms)?;
    Ok(())
}

/// Recursively make all files in a directory writable (0644)
pub fn make_tree_writable(dir: &Path) -> Result<()> {
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

/// Make a single existing file writable so it can be overwritten
pub fn make_file_writable(path: &Path) -> Result<()> {
    if path.exists() {
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o644);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

/// Write content to a file and set it read-only. Creates parent dirs as needed.
pub fn write_readonly(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    make_file_writable(path)?;
    fs::write(path, content)?;
    set_readonly(path)?;
    Ok(())
}

/// Remove a placed path (file or directory), handling read-only permissions
pub fn remove_placed(workspace: &Path, rel_path: &str) -> Result<()> {
    let full_path = workspace.join(rel_path);
    if full_path.is_dir() {
        make_tree_writable(&full_path)?;
        fs::remove_dir_all(&full_path)?;
        println!("    Removed directory: {}", rel_path);
    } else if full_path.is_file() {
        make_file_writable(&full_path)?;
        fs::remove_file(&full_path)?;
        println!("    Removed file: {}", rel_path);
    }
    Ok(())
}

/// Detect an agent by checking for a config directory or a binary in PATH
pub fn detect_by_dir_or_binary(workspace: &Path, config_dir: &str, binary_name: &str) -> bool {
    if workspace.join(config_dir).is_dir() {
        return true;
    }
    std::process::Command::new("which")
        .arg(binary_name)
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Generate the standard pallet numeric-prefixed filename:
/// `{NN}-{source_name}-{filename}`
pub fn prefixed_filename(source_index: usize, source_name: &str, filename: &str) -> String {
    format!("{:02}-{}-{}", source_index, source_name, filename)
}

/// Place a skill (Directory resource) into {skills_dir}/{name}/ as an Agent Skills directory.
/// Reusable across all adapters that support the SKILL.md standard.
/// `agent_prefix` is the agent-specific path prefix for display/tracking, e.g. ".claude", ".goose".
pub fn place_skill_directory(
    skills_dir: &Path,
    resource: &RawResource,
    agent_prefix: &str,
    hashes: &mut HashMap<String, String>,
    placed_paths: &mut Vec<String>,
) -> Result<()> {
    let skill_dir = skills_dir.join(&resource.name);

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
            hashes.insert(format!("skills/{}/{}", resource.name, relative_path), hash);
        }
    }

    let placed = format!("{}/skills/{}", agent_prefix, resource.name);
    println!("    Skill '{}': {}", resource.name, placed);
    placed_paths.push(placed);

    Ok(())
}

/// Place a single-file resource as a plain markdown file (frontmatter stripped).
/// Used by agents without native frontmatter support (Goose, OpenCode, Codex).
/// `target_dir` is the directory to write into, `dir_prefix` is for display/tracking.
pub fn place_rule_as_plain_md(
    target_dir: &Path,
    resource: &RawResource,
    dir_prefix: &str,
    hashes: &mut HashMap<String, String>,
    placed_paths: &mut Vec<String>,
) -> Result<()> {
    let filename = match &resource.content {
        ResourceContent::SingleFile { filename, .. } => filename.clone(),
        _ => format!("{}.md", resource.name),
    };

    let placed_name = prefixed_filename(resource.source_index, &resource.source_name, &filename);
    let file_path = target_dir.join(&placed_name);

    if let ResourceContent::SingleFile { content, .. } = &resource.content {
        let text = String::from_utf8_lossy(content);
        let body = strip_frontmatter(&text);
        let output = body.trim().as_bytes();
        write_readonly(&file_path, output)?;

        let hash = store::sha256_hex(output);
        hashes.insert(format!("{}/{}", dir_prefix, placed_name), hash);
    }

    let placed = format!("{}/{}", dir_prefix, placed_name);
    println!("    {} '{}': {}", resource.kind, resource.name, placed);
    placed_paths.push(placed);

    Ok(())
}

/// Place a built-in pallet skill as a SKILL.md in {skills_dir}/pallet/SKILL.md
pub fn place_builtin_skill(skills_dir: &Path, agent_prefix: &str) -> Result<()> {
    let skill_path = skills_dir.join("pallet").join("SKILL.md");
    write_readonly(&skill_path, crate::builtin::PALLET_SKILL.as_bytes())?;
    println!(
        "    Built-in skill 'pallet': {}/skills/pallet/SKILL.md",
        agent_prefix
    );
    Ok(())
}

/// Strip YAML frontmatter (--- delimited) from markdown content
pub fn strip_frontmatter(content: &str) -> &str {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content;
    }
    let rest = &trimmed[3..];
    if let Some(end) = rest.find("\n---") {
        let after = &rest[end + 4..];
        after.trim_start_matches('\n')
    } else {
        content
    }
}
