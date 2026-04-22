use crate::config::SourceConfig;
use crate::git;
use crate::hub::HubClient;
use crate::resource::{RawResource, ResourceContent, ResourceKind};
use crate::source::FetchResult;
use anyhow::Result;
use std::path::Path;

/// Fetch profile resources from the hub
pub async fn fetch(
    source: &SourceConfig,
    workspace: &Path,
    source_index: usize,
    hub_url: &str,
    hub_token: &str,
) -> Result<FetchResult> {
    let client = HubClient::with_token(hub_url, hub_token);

    // Detect workspace git remote
    let workspace_info = match git::detect_workspace(workspace) {
        Ok(info) => info,
        Err(e) => {
            eprintln!("  Warning: Could not detect workspace git info: {e}");
            eprintln!("  Skipping hub profile sync (no git remote to match)");
            return Ok(FetchResult {
                resources: Vec::new(),
                resolved_ref: None,
            });
        }
    };

    println!(
        "  Workspace remote: {} (branch: {})",
        workspace_info.remote_url, workspace_info.branch
    );

    // List applications and find matching one
    let apps = client.list_applications().await?;
    let matched = match_application(&apps, &workspace_info);

    let app = match matched {
        Some(app) => {
            println!(
                "  Matched hub application: {} (id: {})",
                app.name, app.id
            );
            app
        }
        None => {
            println!("  No hub application matches workspace remote");
            return Ok(FetchResult {
                resources: Vec::new(),
                resolved_ref: None,
            });
        }
    };

    // List profiles for the matched application
    let profiles = client.list_profiles(app.id).await?;
    if profiles.is_empty() {
        println!(
            "  No analysis profiles found for application '{}'",
            app.name
        );
        return Ok(FetchResult {
            resources: Vec::new(),
            resolved_ref: None,
        });
    }

    println!("  Found {} profile(s)", profiles.len());

    let mut resources = Vec::new();
    let mut resolved_ref_parts = Vec::new();

    for profile in &profiles {
        println!(
            "  Found profile: {} (id: {})",
            profile.name, profile.id
        );

        resolved_ref_parts.push(format!("profile:{}:v{}", profile.id, profile.id));
        resources.push(RawResource {
            name: profile.name.clone(),
            kind: ResourceKind::Profile,
            source_name: source.name.clone(),
            source_index,
            governance: "governed".to_string(),
            content: ResourceContent::ProfileBundle,
            globs: None,
            description: None,
        });
    }

    let resolved_ref = if resolved_ref_parts.is_empty() {
        None
    } else {
        Some(resolved_ref_parts.join(","))
    };

    Ok(FetchResult {
        resources,
        resolved_ref,
    })
}

/// Match a hub application by comparing normalized git remote URLs
fn match_application(
    apps: &[crate::hub::Application],
    workspace_info: &git::WorkspaceInfo,
) -> Option<crate::hub::Application> {
    let workspace_remote = &workspace_info.remote_url;

    // First pass: match by remote URL
    let mut candidates: Vec<_> = apps
        .iter()
        .filter(|app| {
            app.repository.as_ref().is_some_and(|repo| {
                repo.url
                    .as_ref()
                    .is_some_and(|url| git::normalize_git_url(url) == *workspace_remote)
            })
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }

    // Multiple matches: filter by branch
    let branch_matches: Vec<_> = candidates
        .iter()
        .filter(|app| {
            app.repository.as_ref().is_some_and(|repo| {
                repo.branch
                    .as_ref()
                    .is_some_and(|b| *b == workspace_info.branch)
            })
        })
        .collect();

    if branch_matches.len() == 1 {
        return Some((**branch_matches[0]).clone());
    }

    // Multiple matches still: filter by path
    if !workspace_info.workspace_relative_path.is_empty() {
        let path_matches: Vec<_> = candidates
            .iter()
            .filter(|app| {
                app.repository.as_ref().is_some_and(|repo| {
                    repo.path
                        .as_ref()
                        .is_some_and(|p| *p == workspace_info.workspace_relative_path)
                })
            })
            .collect();

        if path_matches.len() == 1 {
            return Some((**path_matches[0]).clone());
        }
    }

    // Fall back to first match
    Some(candidates.remove(0).clone())
}

