//! Minimal REST client: the calls core parity needs (`lib/api.ts`).

use std::time::Duration;

use corvane_models::{Account, GitHubRepository};
use serde::Deserialize;
use tracing::debug;

use crate::USER_AGENT;
use crate::endpoint::Endpoint;
use crate::error::{GitHubError, Result};

pub struct Client {
    agent: ureq::Agent,
    endpoint: Endpoint,
    token: String,
}

#[derive(Debug, Deserialize)]
struct ApiUser {
    id: u64,
    login: String,
    name: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiEmail {
    email: String,
    primary: bool,
    verified: bool,
}

#[derive(Debug, Deserialize)]
pub struct ApiRepository {
    pub name: String,
    pub owner: ApiOwner,
    pub html_url: String,
    pub clone_url: String,
    pub default_branch: Option<String>,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub fork: bool,
    pub parent: Option<Box<ApiRepository>>,
    #[serde(default)]
    pub pushed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiOwner {
    pub login: String,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: Option<String>,
}

impl Client {
    pub fn new(endpoint: Endpoint, token: impl Into<String>) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .user_agent(USER_AGENT)
            .build()
            .new_agent();
        Self {
            agent,
            endpoint,
            token: token.into(),
        }
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = self.endpoint.api(path);
        debug!(%url, "GET");
        let mut response = self
            .agent
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .call()?;
        let status = response.status().as_u16();
        if status == 401 {
            return Err(GitHubError::Auth("token rejected".into()));
        }
        if !(200..300).contains(&status) {
            let message = response
                .body_mut()
                .read_json::<ApiError>()
                .ok()
                .and_then(|e| e.message)
                .unwrap_or_else(|| "request failed".into());
            return Err(GitHubError::Api { status, message });
        }
        Ok(response.body_mut().read_json()?)
    }

    /// `GET /user` (+ `/user/emails` when the scope allows) → `Account`.
    pub fn current_user(&self, scopes: Vec<String>) -> Result<Account> {
        let user: ApiUser = self.get_json("user")?;
        let emails = match self.get_json::<Vec<ApiEmail>>("user/emails") {
            Ok(list) => {
                let mut list: Vec<ApiEmail> = list.into_iter().filter(|e| e.verified).collect();
                list.sort_by_key(|e| !e.primary);
                list.into_iter().map(|e| e.email).collect()
            }
            Err(err) => {
                debug!(?err, "could not read emails");
                Vec::new()
            }
        };
        Ok(Account {
            endpoint: self.endpoint.api_base.clone(),
            id: user.id,
            login: user.login,
            name: user.name,
            avatar_url: user.avatar_url,
            emails,
            scopes,
        })
    }

    /// `GET /repos/{owner}/{name}`.
    pub fn repository(&self, owner: &str, name: &str) -> Result<GitHubRepository> {
        let repo: ApiRepository = self.get_json(&format!("repos/{owner}/{name}"))?;
        Ok(self.convert(repo))
    }

    /// `GET /user/repos` (all pages, newest pushed first) for the Clone dialog.
    pub fn user_repositories(&self) -> Result<Vec<GitHubRepository>> {
        let mut out = Vec::new();
        for page in 1..=20u32 {
            let batch: Vec<ApiRepository> = self.get_json(&format!(
                "user/repos?per_page=100&sort=pushed&affiliation=owner,collaborator,organization_member&page={page}"
            ))?;
            let done = batch.len() < 100;
            out.extend(batch.into_iter().map(|r| self.convert(r)));
            if done {
                break;
            }
        }
        Ok(out)
    }

    fn convert(&self, repo: ApiRepository) -> GitHubRepository {
        GitHubRepository {
            endpoint: self.endpoint.api_base.clone(),
            owner: repo.owner.login,
            name: repo.name,
            html_url: repo.html_url,
            clone_url: repo.clone_url,
            default_branch: repo.default_branch,
            private: repo.private,
            fork: repo.fork,
            parent: repo.parent.map(|p| Box::new(self.convert(*p))),
        }
    }
}
