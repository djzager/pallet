use crate::config::{self, SourceConfig};
use crate::resource::{self, RawResource, ResourceContent, ResourceKind};
use crate::source::FetchResult;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Fetch resources from a git source
pub async fn fetch(source: &SourceConfig, source_index: usize, skip_pull: bool) -> Result<FetchResult> {
    let url = source
        .url
        .as_ref()
        .context("Git source missing 'url'")?;
    let git_ref = source.git_ref.as_deref().unwrap_or("main");

    // Clone or update cache
    let cache_dir = config::cache_dir()?.join(&source.name);
    if skip_pull {
        if !cache_dir.join(".git").exists() {
            anyhow::bail!(
                "No cached repo for '{}'. Run `pallet sync` first.",
                source.name
            );
        }
    } else {
        clone_or_pull(&cache_dir, url, git_ref)?;
    }

    // Capture resolved commit SHA
    let resolved_ref = git_rev_parse_head(&cache_dir).ok();

    let exclude = source.exclude.as_deref().unwrap_or(&[]);

    // Walk configured paths and discover resources
    let paths = source.paths.as_deref().unwrap_or(&[]);

    let mut resources = Vec::new();

    if paths.is_empty() {
        // Walk entire repo
        discover_resources(
            &cache_dir,
            &cache_dir,
            &source.name,
            source_index,
            &mut resources,
            None,
            exclude,
        )?;
    } else {
        // Check if the repo root has a skill marker that paths: config is bypassing
        if has_skill_marker(&cache_dir) {
            if let Some(marker) = find_primary_skill_marker(&cache_dir) {
                eprintln!(
                    "  Warning: source '{}' has a root {} (designed as a single skill),",
                    source.name, marker
                );
                eprintln!(
                    "    but 'paths' config selects subdirectories which bypasses it."
                );
                eprintln!(
                    "    Consider removing 'paths' to sync the repo as a single on-demand skill."
                );
            }
        }

        for path_entry in paths {
            let sub_path = path_entry.path();
            let full_path = cache_dir.join(sub_path);
            // Use explicit kind hint if provided, otherwise infer from directory name
            let kind_hint = path_entry.kind_hint().cloned().or_else(|| {
                full_path
                    .file_name()
                    .and_then(|n| kind_for_directory(&n.to_string_lossy()))
            });
            if full_path.exists() {
                discover_resources(
                    &full_path,
                    &cache_dir,
                    &source.name,
                    source_index,
                    &mut resources,
                    kind_hint,
                    exclude,
                )?;
            } else {
                eprintln!(
                    "Warning: path '{}' not found in source '{}'",
                    sub_path, source.name
                );
            }
        }
    }

    Ok(FetchResult {
        resources,
        resolved_ref,
    })
}

fn clone_or_pull(cache_dir: &Path, url: &str, git_ref: &str) -> Result<()> {
    if cache_dir.join(".git").exists() {
        // Pull latest
        println!("  Updating cached repo: {}", cache_dir.display());
        let status = Command::new("git")
            .args(["pull", "--ff-only"])
            .current_dir(cache_dir)
            .status()
            .context("Failed to git pull")?;
        if !status.success() {
            // If pull fails, try a fresh clone
            eprintln!("  Pull failed, re-cloning...");
            fs::remove_dir_all(cache_dir)?;
            shallow_clone(cache_dir, url, git_ref)?;
        }
    } else {
        shallow_clone(cache_dir, url, git_ref)?;
    }
    Ok(())
}

fn shallow_clone(cache_dir: &Path, url: &str, git_ref: &str) -> Result<()> {
    println!("  Cloning {} (ref: {})...", url, git_ref);
    if let Some(parent) = cache_dir.parent() {
        fs::create_dir_all(parent)?;
    }
    let status = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            git_ref,
            url,
            &cache_dir.to_string_lossy(),
        ])
        .status()
        .context("Failed to git clone")?;
    if !status.success() {
        anyhow::bail!("git clone failed for {}", url);
    }
    Ok(())
}

/// Get the resolved HEAD commit SHA from a git repo
fn git_rev_parse_head(repo_dir: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .context("Failed to run git rev-parse HEAD")?;
    if !output.status.success() {
        anyhow::bail!("git rev-parse HEAD failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Skill marker filenames in priority order.
/// The first one found becomes the primary content file for the skill.
pub const SKILL_MARKERS: &[&str] = &["SKILL.md", "CLAUDE.md", "AGENTS.md"];

/// Check if a directory contains any skill marker file (SKILL.md, CLAUDE.md, or AGENTS.md)
pub fn has_skill_marker(path: &Path) -> bool {
    SKILL_MARKERS.iter().any(|m| path.join(m).exists())
}

/// Find the primary skill marker file in a directory, resolving symlinks to avoid duplicates.
/// Returns the marker filename (e.g. "SKILL.md") or None.
pub fn find_primary_skill_marker(path: &Path) -> Option<&'static str> {
    SKILL_MARKERS.iter().copied().find(|m| path.join(m).exists())
}

/// Infer a resource kind hint from a well-known directory name
pub fn kind_for_directory(name: &str) -> Option<ResourceKind> {
    match name {
        "skills" => Some(ResourceKind::Skill),
        "rules" => Some(ResourceKind::Rule),
        "agents" => Some(ResourceKind::Agent),
        "prompts" => Some(ResourceKind::Prompt),
        _ => None,
    }
}

/// Check if a path component matches any exclude pattern
pub fn is_excluded(name: &str, exclude: &[String]) -> bool {
    exclude.iter().any(|e| e == name)
}

/// Recursively discover resources under a path
pub fn discover_resources(
    path: &Path,
    repo_root: &Path,
    source_name: &str,
    source_index: usize,
    resources: &mut Vec<RawResource>,
    kind_hint: Option<ResourceKind>,
    exclude: &[String],
) -> Result<()> {
    if !path.is_dir() {
        // Single file — try to parse as a resource
        if path.extension().is_some_and(|e| e == "md") {
            if let Some(resource) =
                try_parse_resource(path, source_name, source_index, kind_hint.as_ref())?
            {
                resources.push(resource);
            }
        }
        return Ok(());
    }

    // Check if this directory is a skill (contains SKILL.md, CLAUDE.md, or AGENTS.md)
    if has_skill_marker(path) {
        let skill = read_skill_directory(path, source_name, source_index)?;
        resources.push(skill);
        return Ok(()); // Don't recurse into skill directories
    }

    // Recurse into subdirectories
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        let entry_name = entry_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Skip .git and excluded directories
        if entry_name == ".git" || is_excluded(&entry_name, exclude) {
            continue;
        }

        if entry_path.is_dir() {
            // Determine kind hint for this subdirectory:
            // 1. Propagate parent's kind_hint if set
            // 2. Otherwise check if the directory name is a well-known convention
            let child_hint = kind_hint.clone().or_else(|| kind_for_directory(&entry_name));

            discover_resources(
                &entry_path,
                repo_root,
                source_name,
                source_index,
                resources,
                child_hint,
                exclude,
            )?;
        } else if entry_path.is_file()
            && entry_path.extension().is_some_and(|e| e == "md")
            && entry_path
                .file_name()
                .is_some_and(|n| n != "SKILL.md" && n != "CLAUDE.md" && n != "AGENTS.md" && n != "README.md")
        {
            if let Some(resource) =
                try_parse_resource(&entry_path, source_name, source_index, kind_hint.as_ref())?
            {
                resources.push(resource);
            }
        }
    }

    Ok(())
}

/// Read a skill directory (SKILL.md + supporting files)
pub fn read_skill_directory(
    dir: &Path,
    source_name: &str,
    source_index: usize,
) -> Result<RawResource> {
    let name = dir
        .file_name()
        .context("Skill directory has no name")?
        .to_string_lossy()
        .to_string();

    let mut files = Vec::new();
    read_dir_recursive(dir, dir, &mut files)?;

    // Parse governance from the primary skill marker's frontmatter
    let governance = find_primary_skill_marker(dir)
        .and_then(|marker| fs::read_to_string(dir.join(marker)).ok())
        .and_then(|content| resource::parse_frontmatter(&content))
        .map(|fm| fm.governance)
        .unwrap_or_else(|| "federated".to_string());

    Ok(RawResource {
        name,
        kind: ResourceKind::Skill,
        source_name: source_name.to_string(),
        source_index,
        governance,
        content: ResourceContent::Directory { files },
    })
}

pub fn read_dir_recursive(
    base: &Path,
    current: &Path,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != ".git") {
                read_dir_recursive(base, &path, files)?;
            }
        } else {
            let relative = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let content = fs::read(&path)?;
            files.push((relative, content));
        }
    }
    Ok(())
}

/// Try to parse a markdown file as a resource.
/// Returns None (with a warning) if no kind can be determined.
pub fn try_parse_resource(
    path: &Path,
    source_name: &str,
    source_index: usize,
    kind_hint: Option<&ResourceKind>,
) -> Result<Option<RawResource>> {
    let content = fs::read(path)?;
    let content_str = String::from_utf8_lossy(&content);

    // Parse frontmatter for type and governance
    let frontmatter = resource::parse_frontmatter(&content_str);

    // Determine resource kind: frontmatter > kind_hint > skip
    let kind = frontmatter
        .as_ref()
        .and_then(|fm| fm.resource_type.as_deref())
        .and_then(ResourceKind::from_str_opt)
        .or_else(|| kind_hint.cloned());

    let kind = match kind {
        Some(k) => k,
        None => {
            let relative = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string());
            eprintln!(
                "  Warning: skipping '{}' — no resource kind (add frontmatter `type:` or use path kind hint)",
                relative
            );
            return Ok(None);
        }
    };

    let governance = frontmatter
        .as_ref()
        .map(|fm| fm.governance.clone())
        .unwrap_or_else(|| "federated".to_string());

    let name = path
        .file_stem()
        .context("File has no stem")?
        .to_string_lossy()
        .to_string();

    let filename = path
        .file_name()
        .context("File has no name")?
        .to_string_lossy()
        .to_string();

    Ok(Some(RawResource {
        name,
        kind,
        source_name: source_name.to_string(),
        source_index,
        governance,
        content: ResourceContent::SingleFile {
            filename,
            content: content.clone(),
        },
    }))
}

