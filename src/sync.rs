use crate::{agent, config, lock, merge, source, store};
use anyhow::{Context, Result};
use std::path::Path;

/// Run the full sync pipeline: fetch → merge → cleanup → place → lock
pub async fn run_sync(workspace: &Path, locked: bool) -> Result<()> {
    // 1. Load config from workspace/pallet.yaml
    let cfg = config::load_config(workspace).context(
        "Failed to load pallet.yaml. Create one with `pallet auth` or `pallet config add-source`.",
    )?;

    println!("Loaded config ({} source(s))", cfg.sources.len());

    // Load hub credentials if needed
    let hub_url = cfg.hub.as_ref().map(|h| h.url.clone());
    let hub_token = if cfg.sources.iter().any(|s| s.source_type == config::SourceType::Hub) {
        let creds = config::load_credentials()
            .context("Hub source configured but no credentials found. Run `pallet auth` first.")?;
        creds.hub_token
    } else {
        None
    };

    // 2. Collect workspace facts
    if let Ok(info) = crate::git::detect_workspace(workspace) {
        println!(
            "Workspace: {} (branch: {}, remote: {})",
            workspace.display(),
            info.branch,
            info.remote_url
        );
    } else {
        println!(
            "Workspace: {} (not a git repo or no remote)",
            workspace.display()
        );
    }

    // 3. Load lock file if --locked mode
    let lock_file = if locked {
        let lf = lock::load_lock(workspace)?;

        // Verify config hash matches
        let config_content = std::fs::read_to_string(config::config_path(workspace))?;
        let config_hash = store::sha256_hex(config_content.as_bytes());
        if config_hash != lf.config_hash {
            anyhow::bail!(
                "Config has changed since lock file was generated. \
                 Run `pallet sync` without --locked to update the lock file."
            );
        }
        println!("Lock file verified (locked at: {})", lf.locked_at);
        Some(lf)
    } else {
        None
    };

    // 4. Fetch from each source
    let mut all_resources = Vec::new();
    let mut source_names = Vec::new();
    let mut fetch_results: Vec<(&config::SourceConfig, Option<String>)> = Vec::new();

    for source_cfg in cfg.sources.iter() {
        println!(
            "\nFetching source: {} ({})",
            source_cfg.name,
            source_cfg.source_type_str()
        );

        // In locked mode, show locked ref info
        if let Some(ref lf) = lock_file {
            if let Some(locked_source) = lf.sources.iter().find(|s| s.name == source_cfg.name) {
                if let Some(ref expected_ref) = locked_source.resolved_ref {
                    println!("  Locked to ref: {}", expected_ref);
                }
            }
        }

        match source::fetch_source(
            source_cfg,
            workspace,
            cfg.sources.iter().position(|s| s.name == source_cfg.name).unwrap_or(0),
            hub_url.as_deref(),
            hub_token.as_deref(),
        )
        .await
        {
            Ok(result) => {
                println!(
                    "  Fetched {} resource(s) from '{}'",
                    result.resources.len(),
                    source_cfg.name
                );
                if let Some(ref sha) = result.resolved_ref {
                    println!("  Resolved ref: {}", sha);
                }
                source_names.push(source_cfg.name.clone());
                fetch_results.push((source_cfg, result.resolved_ref));
                all_resources.extend(result.resources);
            }
            Err(e) => {
                if locked {
                    anyhow::bail!(
                        "Failed to fetch from '{}' in locked mode: {e}",
                        source_cfg.name
                    );
                }
                eprintln!(
                    "  Warning: Failed to fetch from '{}': {e}",
                    source_cfg.name
                );
                fetch_results.push((source_cfg, None));
            }
        }
    }

    if all_resources.is_empty() {
        println!("\nNo resources fetched from any source.");
        return Ok(());
    }

    // 5. Merge with hierarchy
    println!(
        "\nMerging {} resource(s) from {} source(s)...",
        all_resources.len(),
        source_names.len()
    );
    let merge_result = merge::merge_resources(all_resources);
    for warning in &merge_result.warnings {
        eprintln!("  Warning: {}", warning);
    }
    let all_resources = merge_result.resources;
    println!("  {} resource(s) after merge", all_resources.len());

    // 6. Clean up previously-placed resources
    if let Ok(old_lock) = lock::load_lock(workspace) {
        let old_paths = lock::all_placed_paths(&old_lock);
        if !old_paths.is_empty() {
            println!("\nCleaning up {} previously-placed resource(s)...", old_paths.len());
            agent::claude::cleanup_placed(workspace, &old_paths)?;
        }
    }

    // 7. Detect agents and place resources directly
    println!("\nPlacing resources...");
    let mut agents = Vec::new();
    let place_result = if agent::claude::detect(workspace) {
        println!("  Detected agent: Claude Code");
        let result = agent::claude::place(workspace, &all_resources)?;
        agents.push("claude".to_string());
        result
    } else {
        println!("  No agents detected — resources not placed");
        agent::claude::PlaceResult {
            hashes: std::collections::HashMap::new(),
            placed_paths: Vec::new(),
        }
    };

    // 8. Verify hashes in locked mode
    if let Some(ref lf) = lock_file {
        let mut mismatches = Vec::new();
        for locked_res in &lf.resources {
            if locked_res.content_hash.is_empty() {
                continue;
            }
            let prefix = format!("{}/", locked_res.kind);
            let actual = place_result
                .hashes
                .iter()
                .find(|(k, _)| k.starts_with(&prefix) && k.contains(&locked_res.name));
            if let Some((_, actual_hash)) = actual {
                if *actual_hash != locked_res.content_hash {
                    mismatches.push(format!(
                        "{}/{}: expected {}, got {}",
                        locked_res.kind, locked_res.name, locked_res.content_hash, actual_hash
                    ));
                }
            }
        }
        if !mismatches.is_empty() {
            for m in &mismatches {
                eprintln!("  Hash mismatch: {}", m);
            }
            anyhow::bail!(
                "Lock file verification failed: {} hash mismatch(es)",
                mismatches.len()
            );
        }
        println!("  Lock file hash verification passed");
    }

    // 9. Write lock file (only in non-locked mode)
    if lock_file.is_none() {
        let config_content = std::fs::read_to_string(config::config_path(workspace))?;
        let config_hash = store::sha256_hex(config_content.as_bytes());
        let lock = lock::build_lock(
            &fetch_results,
            &all_resources,
            &place_result.hashes,
            &place_result.placed_paths,
            &config_hash,
        );
        lock::save_lock(workspace, &lock)?;
        println!("  Lock file written to pallet.lock");
    }

    // Summary
    println!("\nSync complete:");
    println!("  Sources: {}", source_names.join(", "));
    let mut resource_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for r in &all_resources {
        *resource_counts.entry(r.kind.to_string()).or_insert(0) += 1;
    }
    for (kind, count) in &resource_counts {
        println!("  {kind}s: {count}");
    }
    if !agents.is_empty() {
        println!("  Agents: {}", agents.join(", "));
    }

    Ok(())
}
