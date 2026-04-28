use crate::config::SourceConfig;
use crate::source::git_source;
use crate::source::FetchResult;
use anyhow::Result;
use std::path::Path;

/// Default directories to exclude when scanning the workspace root
const DEFAULT_EXCLUDE: &[&str] = &[
    ".claude",
    ".cursor",
    ".goose",
    ".git",
    "target",
    "node_modules",
    ".pallet",
];

/// Fetch resources from a local source (project-level directories)
pub async fn fetch(
    source: &SourceConfig,
    workspace: &Path,
    source_index: usize,
) -> Result<FetchResult> {
    let exclude = source.exclude.as_deref().unwrap_or(&[]);
    let paths = source.paths.as_deref().unwrap_or(&[]);

    // Build combined exclude list: user-specified + defaults
    let mut all_exclude: Vec<String> = exclude.to_vec();
    for &default in DEFAULT_EXCLUDE {
        if !all_exclude.iter().any(|e| e == default) {
            all_exclude.push(default.to_string());
        }
    }

    let mut resources = Vec::new();

    if paths.is_empty() {
        // Walk workspace root with default exclusions
        git_source::discover_resources(
            workspace,
            workspace,
            &source.name,
            source_index,
            &mut resources,
            None,
            &all_exclude,
        )?;
    } else {
        for path_entry in paths {
            let sub_path = path_entry.path();
            let kind_hint = path_entry.kind_hint().cloned();
            let full_path = workspace.join(sub_path);
            if full_path.exists() {
                git_source::discover_resources(
                    &full_path,
                    workspace,
                    &source.name,
                    source_index,
                    &mut resources,
                    kind_hint,
                    &all_exclude,
                )?;
            } else {
                eprintln!(
                    "Warning: path '{}' not found in workspace for source '{}'",
                    sub_path, source.name
                );
            }
        }
    }

    Ok(FetchResult {
        resources,
        resolved_ref: None,
    })
}
