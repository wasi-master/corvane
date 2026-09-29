//! Repository rules and branch protection - GHD
//! `app-store.ts#refreshBranchProtectionState`, `lib/helpers/repo-rules.ts`
//! (`parseRepoRules`, the metadata matchers) and `models/repo-rules.ts`.
//!
//! Deviation: GHD skips the rules API for private repositories of free
//! accounts (`useRepoRulesLogic`) using the account plan; Corvane always
//! asks and treats "not found" as "no rules".

use std::collections::HashMap;
use std::time::{Duration, Instant};

use corvane_github::{ApiRepoRule, Client};
use corvane_models::{
    GitHubRepository, RepoRuleEnforced, RepoRulesInfo, RepoRulesMetadataFailure,
    RepoRulesMetadataFailures, RepoRulesMetadataRule, RuleOperator, Tip,
};
use gpui_kit::App;
use tracing::warn;

use crate::dispatcher::Dispatcher;
use crate::remote::spawn_bg;

/// Rules are re-fetched for the same branch at most this often.
const RULES_MAX_AGE: Duration = Duration::from_secs(5 * 60);

/// `toMatcher`: does `text` satisfy the rule (negation included)?
pub fn rule_matches(rule: &RepoRulesMetadataRule, text: &str) -> bool {
    let matched = match rule.operator {
        RuleOperator::StartsWith => text.starts_with(&rule.pattern),
        RuleOperator::EndsWith => text.ends_with(&rule.pattern),
        RuleOperator::Contains => text.contains(&rule.pattern),
        RuleOperator::RegexMatch => match regex::Regex::new(&rule.pattern) {
            Ok(re) => re.is_match(text),
            Err(err) => {
                warn!(%err, pattern = %rule.pattern, "unsupported rule pattern");
                return true;
            }
        },
    };
    if rule.negate { !matched } else { matched }
}

/// `RepoRulesMetadataRules.getFailedRules`
pub fn failed_rules(rules: &[RepoRulesMetadataRule], text: &str) -> RepoRulesMetadataFailures {
    let mut failures = RepoRulesMetadataFailures::default();
    for rule in rules {
        if rule_matches(rule, text) {
            continue;
        }
        let failure = RepoRulesMetadataFailure {
            description: rule.human_description(),
            ruleset_id: rule.ruleset_id,
        };
        if rule.enforced == RepoRuleEnforced::Bypass {
            failures.bypassed.push(failure);
        } else {
            failures.failed.push(failure);
        }
    }
    failures
}

/// `parseRepoRules`
pub fn parse_repo_rules(
    rules: &[ApiRepoRule],
    rulesets: &HashMap<u64, RepoRuleEnforced>,
    gpg_sign_enabled: bool,
) -> RepoRulesInfo {
    let mut info = RepoRulesInfo::default();
    let metadata = |rule: &ApiRepoRule, enforced: RepoRuleEnforced| {
        rule.parameters.as_ref().map(|p| RepoRulesMetadataRule {
            enforced,
            ruleset_id: rule.ruleset_id,
            operator: p.operator,
            pattern: p.pattern.clone(),
            negate: p.negate,
        })
    };
    for rule in rules {
        let Some(enforced) = rulesets.get(&rule.ruleset_id).copied() else {
            continue;
        };
        match rule.kind.as_str() {
            "update" | "required_deployments" | "required_status_checks" => {
                info.basic_commit_warning = info.basic_commit_warning.combine(enforced);
            }
            "creation" => info.creation_restricted = info.creation_restricted.combine(enforced),
            "required_signatures" => {
                if !gpg_sign_enabled {
                    info.signed_commits_required = info.signed_commits_required.combine(enforced);
                }
            }
            "pull_request" => {
                info.pull_request_required = info.pull_request_required.combine(enforced)
            }
            "commit_message_pattern" => info
                .commit_message_patterns
                .extend(metadata(rule, enforced)),
            "commit_author_email_pattern" => info
                .commit_author_email_patterns
                .extend(metadata(rule, enforced)),
            "committer_email_pattern" => info
                .committer_email_patterns
                .extend(metadata(rule, enforced)),
            "branch_name_pattern" => info.branch_name_patterns.extend(metadata(rule, enforced)),
            _ => {}
        }
    }
    info
}

/// `findRemoteBranchName`: the branch as GitHub knows it - the upstream
/// name when the branch is published to the repository's remote, the local
/// name when it is unpublished (creation rules), nothing otherwise.
fn remote_branch_name(
    tip: &Tip,
    remote_url: Option<&str>,
    gh: &GitHubRepository,
) -> Option<String> {
    let Tip::Valid { branch } = tip else {
        return None;
    };
    match branch.upstream_short() {
        None => Some(branch.name.clone()),
        Some(upstream) => {
            let matches = remote_url
                .is_some_and(|url| corvane_models::url_matches_remote(url, &gh.clone_url));
            if matches {
                upstream.split_once('/').map(|(_, name)| name.to_string())
            } else {
                None
            }
        }
    }
}

impl Dispatcher {
    /// `refreshBranchProtectionState`: push control + rulesets + branch
    /// rules for the current branch, throttled per branch.
    pub(crate) fn refresh_branch_protection(id: u64, cx: &mut App) {
        let (github, branch, remote_url, prior_rulesets) = {
            let s = Self::state(cx).read(cx);
            let Some(gh) = s.repository(id).and_then(|r| r.github.clone()) else {
                return;
            };
            let Some(rs) = s.repo_states.get(&id) else {
                return;
            };
            let Some(info) = rs.info.as_ref() else {
                return;
            };
            let remote_url = Self::current_remote_in(s, id).map(|r| r.url);
            let Some(branch) = remote_branch_name(&info.tip, remote_url.as_deref(), &gh) else {
                return;
            };
            if rs.repo_rules_branch.as_deref() == Some(branch.as_str())
                && rs
                    .repo_rules_fetched_at
                    .is_some_and(|t| t.elapsed() < RULES_MAX_AGE)
            {
                return;
            }
            (gh, branch, remote_url, s.repo_rulesets.clone())
        };
        let Some((endpoint, token, _)) = Self::api_for(&github, cx) else {
            return;
        };
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        let _ = remote_url;
        Self::state(cx).update(cx, |s, _| {
            let rs = s.repo_state_mut(id);
            rs.repo_rules_branch = Some(branch.clone());
            rs.repo_rules_fetched_at = Some(Instant::now());
        });
        let dotcom = github.endpoint == "https://api.github.com";
        let (owner, name) = (github.owner.clone(), github.name.clone());
        let branch_for_load = branch.clone();
        spawn_bg(
            cx,
            move || {
                let client = Client::new(endpoint, token);
                let protected = client
                    .push_control(&owner, &name, &branch_for_load)
                    .map(|pc| !pc.is_pushable())
                    .unwrap_or(false);
                let mut rulesets = prior_rulesets;
                let mut info = RepoRulesInfo::default();
                if dotcom {
                    if let Ok(Some(slim)) = client.repo_rulesets(&owner, &name) {
                        for r in slim {
                            if rulesets.contains_key(&r.id) {
                                continue;
                            }
                            if let Ok(Some(ruleset)) = client.repo_ruleset(&owner, &name, r.id) {
                                rulesets.insert(r.id, ruleset.enforced());
                            }
                        }
                    }
                    let rules = client
                        .repo_rules_for_branch(&owner, &name, &branch_for_load)
                        .unwrap_or_default();
                    if !rules.is_empty() {
                        let gpg = corvane_git::config_value(git, &workdir, "commit.gpgsign")
                            .is_some_and(|v| v.eq_ignore_ascii_case("true"));
                        info = parse_repo_rules(&rules, &rulesets, gpg);
                    }
                }
                (protected, info, rulesets)
            },
            move |(protected, info, rulesets), cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.repo_rulesets = rulesets;
                    let rs = s.repo_state_mut(id);
                    if rs.repo_rules_branch.as_deref() != Some(branch.as_str()) {
                        return;
                    }
                    rs.current_branch_protected = protected;
                    rs.repo_rules = info;
                    cx.notify();
                });
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corvane_github::api::ApiRepoRuleParameters;

    fn rule(
        kind: &str,
        ruleset_id: u64,
        params: Option<(RuleOperator, &str, bool)>,
    ) -> ApiRepoRule {
        ApiRepoRule {
            ruleset_id,
            kind: kind.into(),
            parameters: params.map(|(operator, pattern, negate)| ApiRepoRuleParameters {
                name: String::new(),
                negate,
                pattern: pattern.into(),
                operator,
            }),
        }
    }

    #[test]
    fn matches_patterns() {
        let starts = RepoRulesMetadataRule {
            enforced: RepoRuleEnforced::Yes,
            ruleset_id: 1,
            operator: RuleOperator::StartsWith,
            pattern: "feat".into(),
            negate: false,
        };
        assert!(rule_matches(&starts, "feat: x"));
        assert!(!rule_matches(&starts, "fix: x"));
        let regex = RepoRulesMetadataRule {
            operator: RuleOperator::RegexMatch,
            pattern: "^[A-Z]+-\\d+".into(),
            negate: true,
            ..starts.clone()
        };
        assert!(!rule_matches(&regex, "ABC-12 done"));
        assert!(rule_matches(&regex, "done"));
        let failures = failed_rules(&[starts.clone(), regex], "fix: y");
        assert_eq!(failures.failed.len(), 1);
        assert_eq!(failures.failed[0].description, "must start with \"feat\"");
    }

    #[test]
    fn parses_rules_with_bypass() {
        let mut rulesets = HashMap::new();
        rulesets.insert(1, RepoRuleEnforced::Yes);
        rulesets.insert(2, RepoRuleEnforced::Bypass);
        let info = parse_repo_rules(
            &[
                rule("update", 2, None),
                rule("required_signatures", 1, None),
                rule("creation", 9, None),
                rule(
                    "commit_message_pattern",
                    1,
                    Some((RuleOperator::Contains, "JIRA", false)),
                ),
            ],
            &rulesets,
            false,
        );
        assert_eq!(info.basic_commit_warning, RepoRuleEnforced::Bypass);
        assert_eq!(info.signed_commits_required, RepoRuleEnforced::Yes);
        assert_eq!(info.creation_restricted, RepoRuleEnforced::No);
        assert_eq!(info.commit_message_patterns.len(), 1);
        let signed = parse_repo_rules(&[rule("required_signatures", 1, None)], &rulesets, true);
        assert_eq!(signed.signed_commits_required, RepoRuleEnforced::No);
    }
}
