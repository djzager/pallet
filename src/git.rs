use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Information about the workspace's git repository
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    pub branch: String,
    pub remote_url: String, // normalized: "github.com/org/repo"
    pub workspace_relative_path: String,
}

/// Detect workspace git info by shelling out to git
pub fn detect_workspace(path: &Path) -> Result<WorkspaceInfo> {
    let repo_root_str = git_cmd(path, &["rev-parse", "--show-toplevel"])?;
    let repo_root = PathBuf::from(repo_root_str.trim());

    let branch = git_cmd(path, &["branch", "--show-current"])
        .unwrap_or_else(|_| "HEAD".to_string())
        .trim()
        .to_string();

    let remote_url_raw = git_cmd(path, &["remote", "get-url", "origin"])
        .unwrap_or_default()
        .trim()
        .to_string();

    let remote_url = normalize_git_url(&remote_url_raw);

    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let workspace_relative_path = canonical
        .strip_prefix(&repo_root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(WorkspaceInfo {
        branch,
        remote_url,
        workspace_relative_path,
    })
}

/// Normalize a git URL to a canonical form: "host/org/repo"
///
/// Handles:
///   https://github.com/org/repo.git  → github.com/org/repo
///   git@github.com:org/repo.git      → github.com/org/repo
///   ssh://git@github.com/org/repo    → github.com/org/repo
///   http://github.com/org/repo       → github.com/org/repo
pub fn normalize_git_url(url: &str) -> String {
    let url = url.trim();
    if url.is_empty() {
        return String::new();
    }

    let mut normalized = url.to_string();

    // Strip trailing .git
    if normalized.ends_with(".git") {
        normalized = normalized[..normalized.len() - 4].to_string();
    }

    // Strip trailing /
    normalized = normalized.trim_end_matches('/').to_string();

    // Handle git@host:org/repo (SCP-style)
    if let Some(rest) = normalized.strip_prefix("git@") {
        // git@github.com:org/repo → github.com/org/repo
        normalized = rest.replacen(':', "/", 1);
        return normalized;
    }

    // Handle ssh://git@host/org/repo
    if let Some(rest) = normalized.strip_prefix("ssh://") {
        let rest = rest.strip_prefix("git@").unwrap_or(rest);
        return rest.to_string();
    }

    // Handle https:// and http://
    if let Some(rest) = normalized.strip_prefix("https://") {
        return rest.to_string();
    }
    if let Some(rest) = normalized.strip_prefix("http://") {
        return rest.to_string();
    }

    normalized
}

fn git_cmd(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .context("Failed to run git")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_https() {
        assert_eq!(
            normalize_git_url("https://github.com/org/repo.git"),
            "github.com/org/repo"
        );
    }

    #[test]
    fn test_normalize_ssh() {
        assert_eq!(
            normalize_git_url("git@github.com:org/repo.git"),
            "github.com/org/repo"
        );
    }

    #[test]
    fn test_normalize_ssh_scheme() {
        assert_eq!(
            normalize_git_url("ssh://git@github.com/org/repo"),
            "github.com/org/repo"
        );
    }

    #[test]
    fn test_normalize_trailing_slash() {
        assert_eq!(
            normalize_git_url("https://github.com/org/repo/"),
            "github.com/org/repo"
        );
    }

    #[test]
    fn test_normalize_plain() {
        assert_eq!(
            normalize_git_url("github.com/org/repo"),
            "github.com/org/repo"
        );
    }
}
