//! Minimal REST client: the calls core parity needs (`lib/api.ts`).

use std::time::Duration;

use corvane_models::{
    Account, BypassReason, CheckConclusion, CheckStatus, GitHubRepository, RepoRuleEnforced,
    RepositoryPermission, RuleOperator,
};
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
    #[serde(default)]
    plan: Option<ApiPlan>,
}

#[derive(Debug, Deserialize)]
struct ApiPlan {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ApiEmail {
    email: String,
    primary: bool,
    verified: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiRepository {
    pub name: String,
    pub owner: ApiOwner,
    pub html_url: String,
    pub clone_url: String,
    #[serde(default)]
    pub ssh_url: Option<String>,
    pub default_branch: Option<String>,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub fork: bool,
    pub parent: Option<Box<ApiRepository>>,
    #[serde(default)]
    pub pushed_at: Option<String>,
    #[serde(default)]
    pub archived: bool,
    /// Only on `GET /repos/{owner}/{name}` with a token (`IAPIRepositoryPermissions`).
    #[serde(default)]
    pub permissions: Option<ApiRepositoryPermissions>,
}

/// `IAPIRepositoryPermissions`
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ApiRepositoryPermissions {
    #[serde(default)]
    pub admin: bool,
    /// aka write
    #[serde(default)]
    pub push: bool,
    /// aka read
    #[serde(default)]
    pub pull: bool,
}

impl ApiRepositoryPermissions {
    /// `getPermissionsString`
    pub fn permission(self) -> Option<RepositoryPermission> {
        if self.admin {
            Some(RepositoryPermission::Admin)
        } else if self.push {
            Some(RepositoryPermission::Write)
        } else if self.pull {
            Some(RepositoryPermission::Read)
        } else {
            None
        }
    }
}

/// A GitHub release (`GET /repos/{owner}/{repo}/releases/tags/{tag}`).
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRelease {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    /// ISO-8601.
    #[serde(default)]
    pub published_at: Option<String>,
    pub html_url: String,
}

/// GHD `IAPIRepositoryCloneInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryCloneInfo {
    pub url: String,
    pub default_branch: Option<String>,
}

/// `state` filter for [`Client::issues`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueState {
    Open,
    Closed,
    All,
}

impl IssueState {
    fn as_str(self) -> &'static str {
        match self {
            IssueState::Open => "open",
            IssueState::Closed => "closed",
            IssueState::All => "all",
        }
    }
}

/// `IAPIIssue` (+ the `pull_request` marker used to filter PRs out).
#[derive(Debug, Clone, Deserialize)]
pub struct ApiIssue {
    pub number: u64,
    pub title: String,
    /// `open` | `closed`
    pub state: String,
    pub updated_at: String,
    #[serde(default)]
    pub pull_request: Option<serde_json::Value>,
}

/// `IAPIFullIdentity` (`GET /users/{login}`), also the `IAPIIdentity` on
/// reviews and comments.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApiIdentity {
    pub id: u64,
    pub login: String,
    /// The profile page (`IAPIIdentity.html_url`).
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// `type`: `User`, `Organization` or `Bot`.
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

/// `IAPIPullRequestReview.state`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiPullRequestReviewState {
    Approved,
    Dismissed,
    Pending,
    Commented,
    ChangesRequested,
}

/// `IAPIPullRequestReview` (`GET /repos/{owner}/{repo}/pulls/{n}/reviews/{id}`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApiPullRequestReview {
    pub id: u64,
    pub user: ApiIdentity,
    /// Empty for a review without a summary comment.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub body: String,
    pub html_url: String,
    pub submitted_at: String,
    pub state: ApiPullRequestReviewState,
}

/// `IAPIComment`: an issue comment on a pull request, or a review comment
/// (`GET /repos/{owner}/{repo}/issues/comments/{id}`,
/// `…/pulls/comments/{id}`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ApiIssueComment {
    pub id: u64,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub body: String,
    pub html_url: String,
    pub user: ApiIdentity,
    pub created_at: String,
}

/// `null` → `""` for bodies the API may send as `null`.
fn null_as_empty<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

/// `IAPIMentionableUser`.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiMentionableUser {
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiOwner {
    pub login: String,
}

/// `IAPIPullRequestRef`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiPullRequestRef {
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub sha: String,
    /// `null` when the head repository was deleted.
    pub repo: Option<ApiRepository>,
}

/// `IAPIPullRequest`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiPullRequest {
    pub number: u64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub user: ApiOwner,
    pub head: ApiPullRequestRef,
    pub base: ApiPullRequestRef,
    #[serde(default)]
    pub body: Option<String>,
    /// `open` | `closed`
    pub state: String,
    #[serde(default)]
    pub draft: bool,
}

/// `IAPIRefStatusItem` (the legacy commit status API).
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRefStatusItem {
    pub id: u64,
    /// `success` | `pending` | `failure` | `error`
    pub state: String,
    #[serde(default)]
    pub target_url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub context: String,
}

/// `IAPIRefStatus`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRefStatus {
    pub state: String,
    #[serde(default)]
    pub total_count: u64,
    #[serde(default)]
    pub statuses: Vec<ApiRefStatusItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiCheckSuiteRef {
    pub id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiCheckApp {
    #[serde(default)]
    pub name: String,
}

/// `IAPIRefCheckRun`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRefCheckRun {
    pub id: u64,
    pub status: CheckStatus,
    #[serde(default)]
    pub conclusion: Option<CheckConclusion>,
    pub name: String,
    #[serde(default)]
    pub check_suite: Option<ApiCheckSuiteRef>,
    #[serde(default)]
    pub app: Option<ApiCheckApp>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub pull_requests: Vec<serde_json::Value>,
}

/// `IAPIRefCheckRuns`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRefCheckRuns {
    #[serde(default)]
    pub total_count: u64,
    #[serde(default)]
    pub check_runs: Vec<ApiRefCheckRun>,
}

/// `IAPICheckSuite`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiCheckSuite {
    pub id: u64,
    #[serde(default)]
    pub rerequestable: bool,
    #[serde(default)]
    pub runs_rerequestable: bool,
    pub status: CheckStatus,
    pub created_at: String,
}

/// `IAPIWorkflowRun`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiWorkflowRun {
    pub id: u64,
    pub workflow_id: u64,
    #[serde(default)]
    pub name: String,
    pub created_at: String,
    #[serde(default)]
    pub check_suite_id: Option<u64>,
    #[serde(default)]
    pub event: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiWorkflowRuns {
    #[serde(default)]
    pub total_count: u64,
    #[serde(default)]
    pub workflow_runs: Vec<ApiWorkflowRun>,
}

/// `IAPIWorkflowJobStep`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiWorkflowJobStep {
    pub name: String,
    pub number: u64,
    pub status: CheckStatus,
    #[serde(default)]
    pub conclusion: Option<CheckConclusion>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
}

/// `IAPIWorkflowJob`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiWorkflowJob {
    pub id: u64,
    pub name: String,
    pub status: CheckStatus,
    #[serde(default)]
    pub conclusion: Option<CheckConclusion>,
    #[serde(default)]
    pub steps: Vec<ApiWorkflowJobStep>,
    #[serde(default)]
    pub html_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiWorkflowJobs {
    #[serde(default)]
    pub total_count: u64,
    #[serde(default)]
    pub jobs: Vec<ApiWorkflowJob>,
}

/// `IAPIPushControl`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiPushControl {
    #[serde(default)]
    pub required_status_checks: Vec<String>,
    #[serde(default)]
    pub required_approving_review_count: u64,
    #[serde(default = "default_true")]
    pub allow_actor: bool,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub required_signatures: bool,
    #[serde(default)]
    pub required_linear_history: bool,
    #[serde(default = "default_true")]
    pub allow_deletions: bool,
    #[serde(default = "default_true")]
    pub allow_force_pushes: bool,
}

fn default_true() -> bool {
    true
}

impl ApiPushControl {
    /// A branch nobody can push to directly (`isBranchPushable` negated).
    pub fn is_pushable(&self) -> bool {
        self.allow_actor
            && self.required_status_checks.is_empty()
            && self.required_approving_review_count == 0
    }
}

impl Default for ApiPushControl {
    fn default() -> Self {
        Self {
            required_status_checks: Vec::new(),
            required_approving_review_count: 0,
            allow_actor: true,
            pattern: None,
            required_signatures: false,
            required_linear_history: false,
            allow_deletions: true,
            allow_force_pushes: true,
        }
    }
}

/// `IAPIRepoRuleMetadataParameters`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRepoRuleParameters {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub negate: bool,
    pub pattern: String,
    pub operator: RuleOperator,
}

/// `IAPIRepoRule`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRepoRule {
    pub ruleset_id: u64,
    /// `creation` | `update` | `required_signatures` | `commit_message_pattern` | …
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub parameters: Option<ApiRepoRuleParameters>,
}

/// `IAPISlimRepoRuleset`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiSlimRepoRuleset {
    pub id: u64,
}

/// `IAPIRepoRuleset`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiRepoRuleset {
    pub id: u64,
    /// `always` | `pull_requests_only` | `never`
    #[serde(default)]
    pub current_user_can_bypass: Option<String>,
}

impl ApiRepoRuleset {
    pub fn enforced(&self) -> RepoRuleEnforced {
        if self.current_user_can_bypass.as_deref() == Some("always") {
            RepoRuleEnforced::Bypass
        } else {
            RepoRuleEnforced::Yes
        }
    }
}

/// `IAPICreatePushProtectionBypassResponse`
#[derive(Debug, Clone, Deserialize)]
pub struct ApiPushProtectionBypass {
    pub reason: String,
    #[serde(default)]
    pub expire_at: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
}

/// `encodeURIComponent` for refs and branch names in API paths.
pub fn encode_path_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
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
        self.get_json_accept(path, "application/vnd.github+json")
    }

    /// `GET` with a specific `Accept` header (the preview APIs).
    fn get_json_accept<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        accept: &str,
    ) -> Result<T> {
        let url = self.endpoint.api(path);
        debug!(%url, "GET");
        let mut request = self
            .agent
            .get(&url)
            .header("Accept", accept)
            .header("X-GitHub-Api-Version", "2022-11-28");
        // an empty token is GHD's `Account.anonymous()`: no Authorization
        if !self.token.is_empty() {
            request = request.header("Authorization", &format!("Bearer {}", self.token));
        }
        let mut response = request.call()?;
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
            plan: user.plan.map(|p| p.name),
        })
    }

    /// `GET` whose non-2xx answers (other than 401) mean "not available"
    /// rather than an error (`fetchCombinedRefStatus`, `fetchRefCheckRuns`…).
    fn get_json_opt<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        accept: &str,
    ) -> Result<Option<T>> {
        match self.get_json_accept::<T>(path, accept) {
            Ok(value) => Ok(Some(value)),
            Err(GitHubError::Api { status, message }) => {
                debug!(status, %message, %path, "not available");
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }

    /// `POST` without a body; `Ok(true)` for a 2xx answer.
    fn post_empty(&self, path: &str) -> Result<bool> {
        let url = self.endpoint.api(path);
        debug!(%url, "POST");
        let response = self
            .agent
            .post(&url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send_empty()?;
        let status = response.status().as_u16();
        if status == 401 {
            return Err(GitHubError::Auth("token rejected".into()));
        }
        Ok((200..300).contains(&status))
    }

    fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let url = self.endpoint.api(path);
        debug!(%url, "POST");
        let mut response = self
            .agent
            .post(&url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send_json(body)?;
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

    /// `GET /user/orgs`: organisations the user can publish to.
    pub fn user_orgs(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Org {
            login: String,
        }
        let orgs: Vec<Org> = self.get_json("user/orgs?per_page=100")?;
        let mut logins: Vec<String> = orgs.into_iter().map(|o| o.login).collect();
        logins.sort_by_key(|l| l.to_lowercase());
        Ok(logins)
    }

    /// `POST /user/repos` or `/orgs/{org}/repos` (GHD `createRepository`).
    pub fn create_repository(
        &self,
        org: Option<&str>,
        name: &str,
        description: &str,
        private: bool,
    ) -> Result<GitHubRepository> {
        let path = match org {
            Some(org) => format!("orgs/{org}/repos"),
            None => "user/repos".to_string(),
        };
        let body = serde_json::json!({
            "name": name,
            "description": description,
            "private": private,
        });
        let repo: ApiRepository = self.post_json(&path, &body)?;
        Ok(self.convert(repo))
    }

    /// `GET /repos/{owner}/{name}`.
    pub fn repository(&self, owner: &str, name: &str) -> Result<GitHubRepository> {
        let repo: ApiRepository = self.get_json(&format!("repos/{owner}/{name}"))?;
        Ok(self.convert(repo))
    }

    /// `fetchRepositoryCloneInfo`: the clone URL (SSH when `ssh`) and default
    /// branch of `owner/name`, `None` when the repository is not found (404,
    /// which GitHub also answers for private repositories the token can't see).
    pub fn repository_clone_info(
        &self,
        owner: &str,
        name: &str,
        ssh: bool,
    ) -> Result<Option<RepositoryCloneInfo>> {
        let path = format!(
            "repos/{}/{}",
            encode_path_component(owner),
            encode_path_component(name)
        );
        match self.get_json::<ApiRepository>(&path) {
            Ok(repo) => Ok(Some(RepositoryCloneInfo {
                url: match (ssh, repo.ssh_url) {
                    (true, Some(ssh_url)) => ssh_url,
                    _ => repo.clone_url,
                },
                default_branch: repo.default_branch,
            })),
            Err(GitHubError::Api { status: 404, .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// `GET /repos/{owner}/{name}/releases/tags/{tag}`; `None` when there is
    /// no such release.
    pub fn release_by_tag(&self, owner: &str, name: &str, tag: &str) -> Result<Option<ApiRelease>> {
        let path = format!(
            "repos/{}/{}/releases/tags/{}",
            encode_path_component(owner),
            encode_path_component(name),
            encode_path_component(tag)
        );
        match self.get_json::<ApiRelease>(&path) {
            Ok(release) => Ok(Some(release)),
            Err(GitHubError::Api { status: 404, .. }) => Ok(None),
            Err(err) => Err(err),
        }
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

    /// `fetchIssues`: `GET /repos/{owner}/{name}/issues` (all pages). PRs are
    /// issues too, so anything carrying a `pull_request` key is dropped.
    /// `since` is an ISO-8601 timestamp; with it the API returns every issue
    /// updated at or after that moment (closed ones included, so callers can
    /// prune them).
    pub fn issues(
        &self,
        owner: &str,
        name: &str,
        state: IssueState,
        since: Option<&str>,
    ) -> Result<Vec<ApiIssue>> {
        let mut out = Vec::new();
        for page in 1..=20u32 {
            let mut path = format!(
                "repos/{owner}/{name}/issues?state={}&per_page=100&page={page}",
                state.as_str()
            );
            if let Some(since) = since {
                path.push_str("&since=");
                path.push_str(since);
            }
            let batch: Vec<ApiIssue> = self.get_json(&path)?;
            let done = batch.len() < 100;
            out.extend(batch.into_iter().filter(|i| i.pull_request.is_none()));
            if done {
                break;
            }
        }
        Ok(out)
    }

    /// `fetchUser`: `GET /users/{login}`; `None` when there is no such user.
    pub fn user(&self, login: &str) -> Result<Option<ApiIdentity>> {
        match self.get_json::<ApiIdentity>(&format!("users/{login}")) {
            Ok(user) => Ok(Some(user)),
            Err(GitHubError::Api { status: 404, .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// `fetchMentionables`: `GET /repos/{owner}/{name}/mentionables/users`
    /// (preview API; needs its own `Accept`). `None` when the repository has
    /// no mentionables endpoint (404 for repositories the token can't see).
    pub fn mentionables(&self, owner: &str, name: &str) -> Result<Option<Vec<ApiMentionableUser>>> {
        let url = self
            .endpoint
            .api(&format!("repos/{owner}/{name}/mentionables/users"));
        debug!(%url, "GET");
        let mut response = self
            .agent
            .get(&url)
            .header("Accept", "application/vnd.github.jerry-maguire-preview")
            .header("Authorization", &format!("Bearer {}", self.token))
            .call()?;
        let status = response.status().as_u16();
        match status {
            404 => Ok(None),
            401 => Err(GitHubError::Auth("token rejected".into())),
            200..=299 => Ok(Some(response.body_mut().read_json()?)),
            _ => {
                let message = response
                    .body_mut()
                    .read_json::<ApiError>()
                    .ok()
                    .and_then(|e| e.message)
                    .unwrap_or_else(|| "request failed".into());
                Err(GitHubError::Api { status, message })
            }
        }
    }

    /// `fetchAllOpenPullRequests`: every open pull request, newest page first.
    pub fn open_pull_requests(&self, owner: &str, name: &str) -> Result<Vec<ApiPullRequest>> {
        let mut out = Vec::new();
        for page in 1..=50u32 {
            let batch: Vec<ApiPullRequest> = self.get_json(&format!(
                "repos/{owner}/{name}/pulls?state=open&per_page=100&page={page}"
            ))?;
            let done = batch.len() < 100;
            out.extend(batch);
            if done {
                break;
            }
        }
        Ok(out)
    }

    /// `fetchUpdatedPullRequests`: pull requests (open and closed) updated
    /// after `since`, most recently updated first. `None` once more than
    /// `max_results` came back (`MaxResultsError`): the caller refetches
    /// the open list instead.
    pub fn pull_requests_updated_since(
        &self,
        owner: &str,
        name: &str,
        since: &str,
        max_results: usize,
    ) -> Result<Option<Vec<ApiPullRequest>>> {
        let mut out: Vec<ApiPullRequest> = Vec::new();
        for page in 1..=50u32 {
            let batch: Vec<ApiPullRequest> = self.get_json(&format!(
                "repos/{owner}/{name}/pulls?state=all&sort=updated&direction=desc&per_page=100&page={page}"
            ))?;
            let done = batch.len() < 100
                || batch
                    .last()
                    .is_some_and(|last| last.updated_at.as_str() <= since);
            out.extend(
                batch
                    .into_iter()
                    .filter(|pr| pr.updated_at.as_str() > since),
            );
            if out.len() >= max_results {
                return Ok(None);
            }
            if done {
                break;
            }
        }
        Ok(Some(out))
    }

    /// `fetchCombinedRefStatus`: `GET /repos/{o}/{n}/commits/{ref}/status`.
    pub fn combined_ref_status(
        &self,
        owner: &str,
        name: &str,
        git_ref: &str,
    ) -> Result<Option<ApiRefStatus>> {
        let safe = encode_path_component(git_ref);
        self.get_json_opt(
            &format!("repos/{owner}/{name}/commits/{safe}/status?per_page=100"),
            "application/vnd.github+json",
        )
    }

    /// `fetchRefCheckRuns`: `GET /repos/{o}/{n}/commits/{ref}/check-runs`.
    pub fn ref_check_runs(
        &self,
        owner: &str,
        name: &str,
        git_ref: &str,
    ) -> Result<Option<ApiRefCheckRuns>> {
        let safe = encode_path_component(git_ref);
        self.get_json_opt(
            &format!("repos/{owner}/{name}/commits/{safe}/check-runs?per_page=100"),
            "application/vnd.github.antiope-preview+json",
        )
    }

    /// `fetchPRActionWorkflowRunByCheckSuiteId`
    pub fn workflow_run_by_check_suite(
        &self,
        owner: &str,
        name: &str,
        check_suite_id: u64,
    ) -> Result<Option<ApiWorkflowRun>> {
        let runs: Option<ApiWorkflowRuns> = self.get_json_opt(
            &format!(
                "repos/{owner}/{name}/actions/runs?event=pull_request&check_suite_id={check_suite_id}"
            ),
            "application/vnd.github.antiope-preview+json",
        )?;
        Ok(runs.and_then(|r| r.workflow_runs.into_iter().next()))
    }

    /// `fetchPRWorkflowRunsByBranchName`
    pub fn workflow_runs_by_branch(
        &self,
        owner: &str,
        name: &str,
        branch: &str,
    ) -> Result<Option<ApiWorkflowRuns>> {
        let safe = encode_path_component(branch);
        self.get_json_opt(
            &format!("repos/{owner}/{name}/actions/runs?event=pull_request&branch={safe}"),
            "application/vnd.github.antiope-preview+json",
        )
    }

    /// `fetchWorkflowRunJobs`
    pub fn workflow_run_jobs(
        &self,
        owner: &str,
        name: &str,
        run_id: u64,
    ) -> Result<Option<ApiWorkflowJobs>> {
        self.get_json_opt(
            &format!("repos/{owner}/{name}/actions/runs/{run_id}/jobs"),
            "application/vnd.github.antiope-preview+json",
        )
    }

    /// `fetchCheckSuite`
    pub fn check_suite(
        &self,
        owner: &str,
        name: &str,
        check_suite_id: u64,
    ) -> Result<Option<ApiCheckSuite>> {
        self.get_json_opt(
            &format!("repos/{owner}/{name}/check-suites/{check_suite_id}"),
            "application/vnd.github+json",
        )
    }

    /// `rerequestCheckSuite`
    pub fn rerequest_check_suite(&self, owner: &str, name: &str, id: u64) -> Result<bool> {
        self.post_empty(&format!("repos/{owner}/{name}/check-suites/{id}/rerequest"))
    }

    /// `rerunJob`
    pub fn rerun_job(&self, owner: &str, name: &str, job_id: u64) -> Result<bool> {
        self.post_empty(&format!("repos/{owner}/{name}/actions/jobs/{job_id}/rerun"))
    }

    /// `rerunFailedJobs`
    pub fn rerun_failed_jobs(&self, owner: &str, name: &str, run_id: u64) -> Result<bool> {
        self.post_empty(&format!(
            "repos/{owner}/{name}/actions/runs/{run_id}/rerun-failed-jobs"
        ))
    }

    /// `forkRepository`: `POST /repos/{o}/{n}/forks` (202 with the fork).
    pub fn fork_repository(&self, owner: &str, name: &str) -> Result<GitHubRepository> {
        let repo: ApiRepository = self.post_json(
            &format!("repos/{owner}/{name}/forks"),
            &serde_json::json!({}),
        )?;
        Ok(self.convert(repo))
    }

    /// `fetchPushControl`: whether the branch takes direct pushes. The
    /// defaults (pushable) come back when the endpoint has no answer.
    pub fn push_control(&self, owner: &str, name: &str, branch: &str) -> Result<ApiPushControl> {
        let safe = encode_path_component(branch);
        Ok(self
            .get_json_opt(
                &format!("repos/{owner}/{name}/branches/{safe}/push_control"),
                "application/vnd.github.phandalin-preview",
            )?
            .unwrap_or_default())
    }

    /// `fetchAllRepoRulesets`
    pub fn repo_rulesets(
        &self,
        owner: &str,
        name: &str,
    ) -> Result<Option<Vec<ApiSlimRepoRuleset>>> {
        self.get_json_opt(
            &format!("repos/{owner}/{name}/rulesets"),
            "application/vnd.github+json",
        )
    }

    /// `fetchRepoRuleset`
    pub fn repo_ruleset(&self, owner: &str, name: &str, id: u64) -> Result<Option<ApiRepoRuleset>> {
        self.get_json_opt(
            &format!("repos/{owner}/{name}/rulesets/{id}"),
            "application/vnd.github+json",
        )
    }

    /// `fetchRepoRulesForBranch`
    pub fn repo_rules_for_branch(
        &self,
        owner: &str,
        name: &str,
        branch: &str,
    ) -> Result<Vec<ApiRepoRule>> {
        let safe = encode_path_component(branch);
        Ok(self
            .get_json_opt(
                &format!("repos/{owner}/{name}/rules/branches/{safe}"),
                "application/vnd.github+json",
            )?
            .unwrap_or_default())
    }

    /// `createPushProtectionBypass`
    pub fn create_push_protection_bypass(
        &self,
        owner: &str,
        name: &str,
        reason: BypassReason,
        placeholder_id: &str,
    ) -> Result<ApiPushProtectionBypass> {
        self.post_json(
            &format!("repos/{owner}/{name}/secret-scanning/push-protection-bypasses"),
            &serde_json::json!({
                "reason": reason.as_str(),
                "placeholder_id": placeholder_id,
            }),
        )
    }

    /// The API repository → model conversion (endpoint-aware).
    pub fn convert(&self, repo: ApiRepository) -> GitHubRepository {
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
            archived: repo.archived,
            permissions: repo.permissions.and_then(|p| p.permission()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_request_review_deserializes() {
        let json = r#"{
            "id": 80,
            "node_id": "MDE3OlB1bGxSZXF1ZXN0UmV2aWV3ODA=",
            "user": {
                "login": "octocat",
                "id": 1,
                "avatar_url": "https://github.com/images/error/octocat_happy.gif",
                "html_url": "https://github.com/octocat",
                "type": "User"
            },
            "body": "Here is the body for the review.",
            "state": "CHANGES_REQUESTED",
            "html_url": "https://github.com/octocat/Hello-World/pull/12#pullrequestreview-80",
            "submitted_at": "2019-11-17T17:43:43Z",
            "commit_id": "ecdd80bb57125d7ba9641ffaa4d7d2c19d3f3091"
        }"#;
        let review: ApiPullRequestReview = serde_json::from_str(json).expect("review");
        assert_eq!(review.id, 80);
        assert_eq!(review.state, ApiPullRequestReviewState::ChangesRequested);
        assert_eq!(review.user.login, "octocat");
        assert_eq!(
            review.user.html_url.as_deref(),
            Some("https://github.com/octocat")
        );
        assert_eq!(review.user.kind.as_deref(), Some("User"));
        assert_eq!(review.submitted_at, "2019-11-17T17:43:43Z");

        let approved = json
            .replace("CHANGES_REQUESTED", "APPROVED")
            .replace(r#""Here is the body for the review.""#, "null");
        let review: ApiPullRequestReview = serde_json::from_str(&approved).expect("approved");
        assert_eq!(review.state, ApiPullRequestReviewState::Approved);
        assert_eq!(review.body, "");
        for (raw, state) in [
            ("COMMENTED", ApiPullRequestReviewState::Commented),
            ("DISMISSED", ApiPullRequestReviewState::Dismissed),
            ("PENDING", ApiPullRequestReviewState::Pending),
        ] {
            let parsed: ApiPullRequestReviewState =
                serde_json::from_str(&format!("\"{raw}\"")).expect("state");
            assert_eq!(parsed, state);
        }
    }

    #[test]
    fn repository_permissions_map_like_get_permissions_string() {
        let perms = |json: &str| -> Option<RepositoryPermission> {
            serde_json::from_str::<ApiRepositoryPermissions>(json)
                .expect("permissions")
                .permission()
        };
        assert_eq!(
            perms(r#"{"admin":true,"push":true,"pull":true}"#),
            Some(RepositoryPermission::Admin)
        );
        assert_eq!(
            perms(r#"{"admin":false,"push":true,"pull":true,"maintain":true}"#),
            Some(RepositoryPermission::Write)
        );
        assert_eq!(
            perms(r#"{"admin":false,"push":false,"pull":true}"#),
            Some(RepositoryPermission::Read)
        );
        assert_eq!(perms(r#"{}"#), None);

        let repo: ApiRepository = serde_json::from_str(
            r#"{"name":"desktop","owner":{"login":"desktop"},"html_url":"https://github.com/desktop/desktop",
                "clone_url":"https://github.com/desktop/desktop.git","default_branch":"development",
                "permissions":{"admin":false,"push":false,"pull":true}}"#,
        )
        .expect("repository");
        let client = Client::new(crate::Endpoint::github_com(), "");
        let converted = client.convert(repo);
        assert_eq!(converted.permissions, Some(RepositoryPermission::Read));
        assert!(!converted.has_write_permission());
    }

    #[test]
    fn issue_comment_deserializes() {
        let json = r#"{
            "id": 1,
            "node_id": "MDEyOklzc3VlQ29tbWVudDE=",
            "url": "https://api.github.com/repos/octocat/Hello-World/issues/comments/1",
            "html_url": "https://github.com/octocat/Hello-World/issues/1347#issuecomment-1",
            "body": "Me too",
            "user": { "login": "octocat", "id": 1, "avatar_url": null, "html_url": "https://github.com/octocat", "type": "User" },
            "created_at": "2011-04-14T16:00:49Z",
            "updated_at": "2011-04-14T16:00:49Z",
            "author_association": "COLLABORATOR"
        }"#;
        let comment: ApiIssueComment = serde_json::from_str(json).expect("comment");
        assert_eq!(comment.id, 1);
        assert_eq!(comment.body, "Me too");
        assert_eq!(comment.user.login, "octocat");
        assert_eq!(comment.user.avatar_url, None);
        assert_eq!(comment.created_at, "2011-04-14T16:00:49Z");
        assert!(comment.html_url.ends_with("#issuecomment-1"));
    }
}
