mod agent;
mod builtin;
mod config;
mod git;
mod hub;
mod lock;
mod merge;
mod resource;
mod source;
mod store;
mod sync;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pallet", about = "Sync and place AI agent configuration")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate with a Konveyor hub
    Auth {
        /// Hub URL
        hub_url: String,
        /// Username
        #[arg(long)]
        user: String,
        /// Password
        #[arg(long)]
        password: String,
    },
    /// Sync resources from configured sources
    Sync {
        /// Workspace path (defaults to current directory)
        path: Option<PathBuf>,
        /// Reproduce exact state from lock file
        #[arg(long)]
        locked: bool,
        /// Preview what would be placed with context impact report (no files written)
        #[arg(long)]
        dry_run: bool,
        /// Continue even if context budget is exceeded
        #[arg(long)]
        force: bool,
    },
    /// Re-sync without pulling remote sources (offline mode)
    Lock {
        /// Workspace path (defaults to current directory)
        path: Option<PathBuf>,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Add a source to the configuration
    AddSource {
        /// Source name (unique identifier)
        name: String,
        /// Source type (git or hub)
        #[arg(long = "type")]
        source_type: String,
        /// Repository URL (required for git sources)
        #[arg(long)]
        url: Option<String>,
        /// Git ref/branch (defaults to "main")
        #[arg(long = "ref")]
        git_ref: Option<String>,
        /// Paths to include (comma-separated)
        #[arg(long, value_delimiter = ',')]
        paths: Option<Vec<String>>,
        /// Paths to exclude (comma-separated)
        #[arg(long, value_delimiter = ',')]
        exclude: Option<Vec<String>>,
    },
    /// Remove a source from the configuration
    RemoveSource {
        /// Source name to remove
        name: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth {
            hub_url,
            user,
            password,
        } => {
            cmd_auth(&hub_url, &user, &password).await?;
        }
        Commands::Sync {
            path,
            locked,
            dry_run,
            force,
        } => {
            let workspace = path.unwrap_or_else(|| PathBuf::from("."));
            let workspace = workspace.canonicalize()?;
            cmd_sync(&workspace, locked, false, dry_run, force).await?;
        }
        Commands::Lock { path } => {
            let workspace = path.unwrap_or_else(|| PathBuf::from("."));
            let workspace = workspace.canonicalize()?;
            cmd_sync(&workspace, false, true, false, false).await?;
        }
        Commands::Config { action } => {
            let workspace = PathBuf::from(".").canonicalize()?;
            match action {
                ConfigAction::Show => {
                    cmd_config_show(&workspace)?;
                }
                ConfigAction::AddSource {
                    name,
                    source_type,
                    url,
                    git_ref,
                    paths,
                    exclude,
                } => {
                    cmd_config_add_source(
                        &workspace,
                        name,
                        source_type,
                        url,
                        git_ref,
                        paths,
                        exclude,
                    )?;
                }
                ConfigAction::RemoveSource { name } => {
                    cmd_config_remove_source(&workspace, name)?;
                }
            }
        }
    }

    Ok(())
}

async fn cmd_auth(hub_url: &str, user: &str, password: &str) -> Result<()> {
    println!("Authenticating with hub: {hub_url}");

    // 1. Login
    let mut client = hub::HubClient::new(hub_url);
    let login = client.login(user, password).await?;
    println!("Authenticated as: {user}");
    println!("Token expires: {}", login.expiry);

    // 2. Verify connectivity
    let apps = client.list_applications().await?;
    println!("Hub connected: {} applications visible", apps.len());

    // 3. Save credentials to ~/.pallet/credentials.yaml
    let creds = config::Credentials {
        hub_token: Some(login.token),
    };
    config::save_credentials(&creds)?;
    println!("Credentials saved to ~/.pallet/credentials.yaml");

    // 4. Write pallet.yaml in current directory with POC sources
    let workspace = PathBuf::from(".").canonicalize()?;
    let cfg = config::Config {
        hub: Some(config::HubConfig {
            url: hub_url.to_string(),
        }),
        sources: vec![
            config::SourceConfig {
                name: "engineering-toolkit".to_string(),
                source_type: config::SourceType::Git,
                url: Some("https://github.com/gwenneg/claude-engineering-toolkit".to_string()),
                git_ref: Some("main".to_string()),
                paths: Some(vec![config::PathEntry::Simple(
                    "skills/agent-readiness".to_string(),
                )]),
                exclude: None,
            },
            config::SourceConfig {
                name: "hub-profiles".to_string(),
                source_type: config::SourceType::Hub,
                url: None,
                git_ref: None,
                paths: None,
                exclude: None,
            },
        ],
        agents: config::AgentsConfig { auto_detect: true },
    };

    config::save_config(&workspace, &cfg)?;

    // 5. Print summary
    println!("\nConfiguration written to pallet.yaml");
    println!("Sources:");
    for s in &cfg.sources {
        match s.source_type {
            config::SourceType::Git => {
                println!("  - {} (git: {})", s.name, s.url.as_deref().unwrap_or("?"));
                if let Some(paths) = &s.paths {
                    for p in paths {
                        println!("    path: {}", p.path());
                    }
                }
            }
            config::SourceType::Hub => {
                println!("  - {} (hub profile sync)", s.name);
            }
            config::SourceType::Local => {
                print!("  - {} (local", s.name);
                if let Some(paths) = &s.paths {
                    let path_strs: Vec<&str> = paths.iter().map(|p| p.path()).collect();
                    print!(": {}", path_strs.join(", "));
                }
                println!(")");
            }
        }
    }

    Ok(())
}

async fn cmd_sync(
    workspace: &std::path::Path,
    locked: bool,
    offline: bool,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    sync::run_sync(workspace, locked, offline, dry_run, force).await
}

fn cmd_config_show(workspace: &std::path::Path) -> Result<()> {
    let cfg = config::load_config(workspace)?;

    // Print YAML
    let yaml = serde_yaml::to_string(&cfg)?;
    println!("{yaml}");

    // Print summary
    println!("---");
    println!("Sources: {}", cfg.sources.len());
    for (i, s) in cfg.sources.iter().enumerate() {
        println!(
            "  [{}] {} ({}{})",
            i,
            s.name,
            s.source_type_str(),
            s.url.as_ref().map(|u| format!(": {u}")).unwrap_or_default()
        );
    }
    Ok(())
}

fn cmd_config_add_source(
    workspace: &std::path::Path,
    name: String,
    type_str: String,
    url: Option<String>,
    git_ref: Option<String>,
    paths: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
) -> Result<()> {
    let mut cfg = config::load_config(workspace).unwrap_or_else(|_| config::Config {
        hub: None,
        sources: Vec::new(),
        agents: config::AgentsConfig { auto_detect: true },
    });

    if cfg.sources.iter().any(|s| s.name == name) {
        bail!(
            "Source '{}' already exists. Use `pallet config remove-source {}` first.",
            name,
            name
        );
    }

    let source_type = match type_str.as_str() {
        "git" => config::SourceType::Git,
        "hub" => config::SourceType::Hub,
        "local" => config::SourceType::Local,
        _ => bail!(
            "Unknown source type '{}'. Must be 'git', 'hub', or 'local'.",
            type_str
        ),
    };

    if source_type == config::SourceType::Git && url.is_none() {
        bail!("Git sources require --url");
    }

    let path_entries = paths.map(|ps| ps.into_iter().map(config::PathEntry::Simple).collect());

    cfg.sources.push(config::SourceConfig {
        name: name.clone(),
        source_type,
        url,
        git_ref,
        paths: path_entries,
        exclude,
    });

    config::save_config(workspace, &cfg)?;
    println!(
        "Source '{}' added. Run `pallet sync .` to fetch resources.",
        name
    );
    Ok(())
}

fn cmd_config_remove_source(workspace: &std::path::Path, name: String) -> Result<()> {
    let mut cfg = config::load_config(workspace)?;

    let before_len = cfg.sources.len();
    cfg.sources.retain(|s| s.name != name);
    if cfg.sources.len() == before_len {
        bail!("Source '{}' not found.", name);
    }

    config::save_config(workspace, &cfg)?;
    println!("Source '{}' removed.", name);
    Ok(())
}
