use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// Login response from POST /hub/auth/login
#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub expiry: i64,
}

/// Application from GET /hub/applications
#[derive(Debug, Clone, Deserialize)]
pub struct Application {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub repository: Option<Repository>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Repository {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

/// Profile from GET /hub/applications/{id}/analysis/profiles
#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub id: u64,
    pub name: String,
}

/// Hub API client
pub struct HubClient {
    base_url: String,
    token: Option<String>,
    client: reqwest::Client,
}

impl HubClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: None,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_token(base_url: &str, token: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: Some(token.to_string()),
            client: reqwest::Client::new(),
        }
    }

    /// POST /hub/auth/login
    pub async fn login(&mut self, user: &str, password: &str) -> Result<LoginResponse> {
        #[derive(Serialize)]
        struct LoginRequest<'a> {
            user: &'a str,
            password: &'a str,
        }

        let url = format!("{}/hub/auth/login", self.base_url);
        let resp = self
            .client
            .post(&url)
            .json(&LoginRequest { user, password })
            .send()
            .await
            .context("Failed to connect to hub")?;

        if !resp.status().is_success() {
            bail!(
                "Login failed: {} {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }

        let login: LoginResponse = resp
            .json()
            .await
            .context("Failed to parse login response")?;
        self.token = Some(login.token.clone());
        Ok(login)
    }

    fn auth_header(&self) -> Result<String> {
        self.token
            .as_ref()
            .map(|t| format!("Bearer {t}"))
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))
    }

    /// GET /hub/applications (Accept: application/x-yaml)
    pub async fn list_applications(&self) -> Result<Vec<Application>> {
        let url = format!("{}/hub/applications", self.base_url);
        let auth = self.auth_header()?;
        let resp = self
            .client
            .get(&url)
            .header("Authorization", &auth)
            .header("Accept", "application/x-yaml")
            .send()
            .await
            .context("Failed to list applications")?;

        if !resp.status().is_success() {
            bail!(
                "Failed to list applications: {} {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }

        let body = resp.text().await?;
        let apps: Vec<Application> =
            serde_yaml::from_str(&body).context("Failed to parse applications YAML")?;
        Ok(apps)
    }

    /// GET /hub/applications/{id}/analysis/profiles
    pub async fn list_profiles(&self, app_id: u64) -> Result<Vec<Profile>> {
        let url = format!(
            "{}/hub/applications/{}/analysis/profiles",
            self.base_url, app_id
        );
        let auth = self.auth_header()?;
        let resp = self
            .client
            .get(&url)
            .header("Authorization", &auth)
            .send()
            .await
            .context("Failed to list profiles")?;

        if !resp.status().is_success() {
            bail!(
                "Failed to list profiles: {} {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            );
        }

        let profiles: Vec<Profile> = resp.json().await.context("Failed to parse profiles")?;
        Ok(profiles)
    }
}
