pub mod git_source;
pub mod hub_source;
pub mod local_source;

use crate::config::SourceConfig;
use crate::resource::RawResource;
use anyhow::Result;
use std::path::Path;

/// Result of fetching from a source, including resolved version info
pub struct FetchResult {
    pub resources: Vec<RawResource>,
    /// Resolved reference (git commit SHA, hub version, etc.)
    pub resolved_ref: Option<String>,
}

/// Fetch resources from a source based on its type
pub async fn fetch_source(
    source: &SourceConfig,
    workspace: &Path,
    source_index: usize,
    hub_url: Option<&str>,
    hub_token: Option<&str>,
    skip_pull: bool,
) -> Result<FetchResult> {
    match source.source_type {
        crate::config::SourceType::Git => {
            git_source::fetch(source, source_index, skip_pull).await
        }
        crate::config::SourceType::Hub => {
            let url = hub_url.ok_or_else(|| anyhow::anyhow!("Hub URL not configured"))?;
            let token = hub_token.ok_or_else(|| {
                anyhow::anyhow!("Hub credentials not found. Run `pallet auth` first.")
            })?;
            hub_source::fetch(source, workspace, source_index, url, token).await
        }
        crate::config::SourceType::Local => {
            local_source::fetch(source, workspace, source_index).await
        }
    }
}
