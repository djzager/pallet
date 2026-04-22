use crate::{agent, config, lock, merge, resource::ResourceKind, source, store};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

enum ReportMode {
    DryRun,
    PostSync,
}

/// Run the full sync pipeline: fetch -> merge -> cleanup -> place -> lock
pub async fn run_sync(workspace: &Path, locked: bool, offline: bool, dry_run: bool, force: bool) -> Result<()> {
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
        println!("Lock file verified (config hash matches)");
        Some(lf)
    } else {
        None
    };

    // 4. Fetch from each source
    let mut all_resources = Vec::new();
    let mut source_names = Vec::new();
    let mut fetch_results: Vec<(&config::SourceConfig, Option<String>)> = Vec::new();

    for source_cfg in cfg.sources.iter() {
        // In offline mode, skip hub sources (no local cache)
        if offline && source_cfg.source_type == config::SourceType::Hub {
            println!(
                "\nSkipping source: {} (hub, offline mode)",
                source_cfg.name
            );
            fetch_results.push((source_cfg, None));
            continue;
        }

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
            offline,
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

    // 6. Context budget check
    let adapters = agent::all_adapters();
    let mut budget_exceeded = false;
    for adapter in &adapters {
        if !adapter.detect(workspace) {
            continue;
        }
        let always_loaded_bytes: usize = all_resources
            .iter()
            .filter(|r| adapter.is_always_loaded(r))
            .map(|r| r.content_size())
            .sum();
        let always_loaded_count = all_resources
            .iter()
            .filter(|r| adapter.is_always_loaded(r))
            .count();
        let budget = adapter.context_budget_bytes();

        if always_loaded_bytes > budget {
            let estimated_tokens = always_loaded_bytes / 4;
            eprintln!(
                "\n  {} context budget exceeded: {} always-loaded resource(s), \
                 ~{}KB (~{} tokens)",
                adapter.display_name(),
                always_loaded_count,
                always_loaded_bytes / 1024,
                estimated_tokens,
            );
            eprintln!(
                "    Budget: ~{}KB (~{} tokens). Exceeds by ~{}KB.",
                budget / 1024,
                budget / 4,
                (always_loaded_bytes - budget) / 1024,
            );
            eprintln!(
                "    Tip: use 'place_as: skill' in paths config, or remove 'paths' to sync as a single skill."
            );
            budget_exceeded = true;
        }
    }
    if budget_exceeded && !dry_run && !force {
        anyhow::bail!(
            "Context budget exceeded for one or more agents. \
             Use `pallet sync --force` to override, or `pallet sync --dry-run` to preview."
        );
    }
    if budget_exceeded && force {
        eprintln!(
            "\n  Warning: context budget exceeded, continuing due to --force"
        );
    }

    if dry_run {
        print_context_report(ReportMode::DryRun, &adapters, workspace, &all_resources);
        return Ok(());
    }

    // 7. Clean up previously-placed resources (per agent)
    if let Ok(old_lock) = lock::load_lock(workspace) {
        let old_placed = lock::all_placed_paths(&old_lock);
        for adapter in &adapters {
            if let Some(paths) = old_placed.get(adapter.name()) {
                if !paths.is_empty() {
                    println!(
                        "\nCleaning up {} previously-placed path(s) for {}...",
                        paths.len(),
                        adapter.display_name()
                    );
                    adapter.cleanup_placed(workspace, paths)?;
                }
            }
        }
    }

    // 8. Detect agents and place resources
    println!("\nPlacing resources...");
    let mut agent_results: HashMap<String, agent::PlaceResult> = HashMap::new();
    let mut detected_agents: Vec<String> = Vec::new();

    for adapter in &adapters {
        if adapter.detect(workspace) {
            println!("  Detected agent: {}", adapter.display_name());
            let result = adapter.place(workspace, &all_resources)?;
            println!("    Placed {} path(s)", result.placed_paths.len());
            detected_agents.push(adapter.name().to_string());
            agent_results.insert(adapter.name().to_string(), result);
        }
    }

    if detected_agents.is_empty() {
        println!("  No agents detected — resources not placed");
    }

    // 9. Verify hashes in locked mode
    if let Some(ref lf) = lock_file {
        let mut mismatches = Vec::new();
        for locked_res in &lf.resources {
            if locked_res.content_hash.is_empty() {
                continue;
            }
            let prefix = format!("{}/", locked_res.kind);
            let actual = agent_results
                .values()
                .flat_map(|pr| pr.hashes.iter())
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

    // 10. Write lock file (only in non-locked mode)
    if lock_file.is_none() {
        let config_content = std::fs::read_to_string(config::config_path(workspace))?;
        let config_hash = store::sha256_hex(config_content.as_bytes());
        let lock = lock::build_lock(
            &fetch_results,
            &all_resources,
            &agent_results,
            &config_hash,
        );
        lock::save_lock(workspace, &lock)?;
        println!("  Lock file written to pallet.lock");
    }

    // Summary
    println!("\nSync complete:");
    println!("  Sources: {}", source_names.join(", "));
    let mut resource_counts: HashMap<String, usize> = HashMap::new();
    for r in &all_resources {
        *resource_counts.entry(r.kind.to_string()).or_insert(0) += 1;
    }
    for (kind, count) in &resource_counts {
        println!("  {kind}s: {count}");
    }
    if !detected_agents.is_empty() {
        println!("  Agents: {}", detected_agents.join(", "));
    }

    // Post-sync context impact report
    print_context_report(ReportMode::PostSync, &adapters, workspace, &all_resources);

    Ok(())
}

/// Print a context impact report for each detected agent
fn print_context_report(
    mode: ReportMode,
    adapters: &[Box<dyn agent::AgentAdapter>],
    workspace: &Path,
    resources: &[crate::resource::RawResource],
) {
    let header = match mode {
        ReportMode::DryRun => "--- Dry run report (no files written) ---",
        ReportMode::PostSync => "--- Context impact ---",
    };
    println!("\n{header}");

    for adapter in adapters {
        if !adapter.detect(workspace) {
            continue;
        }
        println!("\n  {}:", adapter.display_name());

        // Always-loaded resources
        let mut always_count = 0usize;
        let mut always_bytes = 0usize;
        for r in resources {
            if adapter.is_always_loaded(r) {
                always_count += 1;
                always_bytes += r.content_size();
            }
        }

        // On-demand resources (conditional or skills)
        let mut ondemand_count = 0usize;
        let mut ondemand_bytes = 0usize;
        for r in resources {
            if !adapter.is_always_loaded(r) {
                match r.kind {
                    ResourceKind::Skill | ResourceKind::Agent | ResourceKind::Rule => {
                        ondemand_count += 1;
                        ondemand_bytes += r.content_size();
                    }
                    _ => {}
                }
            }
        }

        let budget = adapter.context_budget_bytes();
        let over = always_bytes > budget;

        println!(
            "    Always-loaded: {} resource(s), ~{}KB (~{} tokens){}",
            always_count,
            always_bytes / 1024,
            always_bytes / 4,
            if over { " << OVER BUDGET" } else { "" },
        );

        // Break down by source
        let mut by_source: HashMap<&str, (usize, usize)> = HashMap::new();
        for r in resources {
            if adapter.is_always_loaded(r) {
                let entry = by_source.entry(&r.source_name).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += r.content_size();
            }
        }
        for (source, (count, bytes)) in &by_source {
            println!(
                "      {}: {} resource(s), ~{}KB",
                source, count, bytes / 1024
            );
        }

        if ondemand_count > 0 {
            println!(
                "    On-demand: {} resource(s), ~{}KB (no startup cost)",
                ondemand_count,
                ondemand_bytes / 1024,
            );
        }

        println!(
            "    Budget: ~{}KB (~{} tokens)",
            budget / 1024,
            budget / 4,
        );
    }
    println!();
}
