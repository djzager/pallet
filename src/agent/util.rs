use crate::resource::{RawResource, ResourceContent, ResourceKind};
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

/// Extract the primary markdown content from a resource, regardless of content type.
/// For SingleFile, returns the content directly.
/// For Directory (skills), finds and returns the SKILL.md content.
pub fn extract_primary_content(resource: &RawResource) -> Option<&[u8]> {
    match &resource.content {
        ResourceContent::SingleFile { content, .. } => Some(content),
        ResourceContent::Directory { files } => {
            // Look for SKILL.md (case-insensitive)
            files
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("SKILL.md"))
                .or_else(|| files.iter().find(|(name, _)| name.ends_with(".md")))
                .map(|(_, content)| content.as_slice())
        }
        ResourceContent::ProfileBundle => None,
    }
}

/// Build a concatenated instructions file from resources.
/// Returns (file_content_bytes, per_resource_hashes).
/// Used by agents that consume a single instructions file (Goose, OpenCode, Codex).
pub fn build_concatenated_instructions(
    resources: &[RawResource],
    builtin_pallet_skill: &str,
) -> (Vec<u8>, HashMap<String, String>) {
    let mut sections: Vec<String> = Vec::new();
    let mut hashes = HashMap::new();

    sections.push("<!-- Managed by pallet. Do not edit. -->".to_string());

    // Group resources by kind
    let mut rules = Vec::new();
    let mut skills = Vec::new();
    let mut agents = Vec::new();

    for resource in resources {
        match resource.kind {
            ResourceKind::Rule => rules.push(resource),
            ResourceKind::Skill => skills.push(resource),
            ResourceKind::Agent => agents.push(resource),
            ResourceKind::Prompt | ResourceKind::Profile => {}
        }
    }

    // Built-in pallet skill
    sections.push(format!(
        "\n## pallet (built-in)\n\n{}",
        builtin_pallet_skill.trim()
    ));

    // Rules section
    if !rules.is_empty() {
        sections.push("\n# Rules\n".to_string());
        for resource in &rules {
            if let Some(content) = extract_primary_content(resource) {
                let text = String::from_utf8_lossy(content);
                // Strip frontmatter for the concatenated output
                let body = strip_frontmatter(&text);
                sections.push(format!(
                    "## {} (from {})\n\n{}",
                    resource.name, resource.source_name, body.trim()
                ));
                let hash = store::sha256_hex(content);
                hashes.insert(
                    format!("rules/{}", resource.name),
                    hash,
                );
            }
        }
    }

    // Skills section
    if !skills.is_empty() {
        sections.push("\n# Skills\n".to_string());
        for resource in &skills {
            if let Some(content) = extract_primary_content(resource) {
                let text = String::from_utf8_lossy(content);
                let body = strip_frontmatter(&text);
                sections.push(format!(
                    "## {} (from {})\n\n{}",
                    resource.name, resource.source_name, body.trim()
                ));
                let hash = store::sha256_hex(content);
                hashes.insert(
                    format!("skills/{}", resource.name),
                    hash,
                );
            }
        }
    }

    // Agents section
    if !agents.is_empty() {
        sections.push("\n# Agents\n".to_string());
        for resource in &agents {
            if let Some(content) = extract_primary_content(resource) {
                let text = String::from_utf8_lossy(content);
                let body = strip_frontmatter(&text);
                sections.push(format!(
                    "## {} (from {})\n\n{}",
                    resource.name, resource.source_name, body.trim()
                ));
                let hash = store::sha256_hex(content);
                hashes.insert(
                    format!("agents/{}", resource.name),
                    hash,
                );
            }
        }
    }

    let content = sections.join("\n\n");
    (content.into_bytes(), hashes)
}

/// Strip YAML frontmatter (--- delimited) from markdown content
fn strip_frontmatter(content: &str) -> &str {
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
