use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, env, process::Command};

fn null_as_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

fn run(args: &[&str]) -> Result<String> {
    let output = Command::new("gh")
        .args(args)
        .output()
        .context("failed to run gh")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let command = command_label(args);
        let detail = stderr.trim();
        if detail.is_empty() {
            anyhow::bail!("{command} failed with {}", output.status);
        }
        anyhow::bail!("{command} failed: {detail}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn command_label(args: &[&str]) -> String {
    let mut label = String::from("gh");
    for arg in args.iter().take(2) {
        label.push(' ');
        label.push_str(arg);
    }
    label
}

#[derive(Debug, Clone, Deserialize)]
pub struct Repo {
    #[serde(alias = "nameWithOwner", alias = "fullName")]
    pub full_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(alias = "updatedAt", alias = "updated_at", default)]
    pub updated_at: Option<String>,
    #[serde(alias = "isPrivate", default)]
    pub is_private: bool,
    #[serde(alias = "stargazerCount", alias = "stargazersCount", default)]
    pub star_count: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Issue {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub author: Option<Author>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(alias = "updatedAt", default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PR {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub author: Option<Author>,
    #[serde(alias = "isDraft", default)]
    pub is_draft: bool,
    #[serde(alias = "updatedAt", default)]
    pub updated_at: Option<String>,
    #[serde(alias = "statusCheckRollup", default)]
    pub status_check_rollup: Vec<CheckRun>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckRun {
    pub name: Option<String>,
    pub status: Option<String>,
    pub conclusion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CheckStatus {
    None,
    Pending,
    Pass,
    Fail,
}

impl PR {
    pub fn overall_check_status(&self) -> CheckStatus {
        if self.status_check_rollup.is_empty() {
            return CheckStatus::None;
        }
        let mut has_pending = false;
        for c in &self.status_check_rollup {
            let status = c.status.as_deref().unwrap_or("");
            let conclusion = c.conclusion.as_deref().unwrap_or("");
            if status != "COMPLETED" {
                has_pending = true;
                continue;
            }
            if matches!(conclusion, "FAILURE" | "TIMED_OUT" | "CANCELLED") {
                return CheckStatus::Fail;
            }
        }
        if has_pending {
            CheckStatus::Pending
        } else {
            CheckStatus::Pass
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct IssueDetail {
    pub number: u32,
    pub title: String,
    pub state: String,
    #[serde(default)]
    pub body: Option<String>,
    pub author: Option<Author>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub comments: Vec<Comment>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Comment {
    pub author: Option<Author>,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Author {
    pub login: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Label {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Notification {
    pub id: String,
    pub reason: String,
    pub subject: NotifSubject,
    pub repository: NotifRepo,
    #[serde(default)]
    pub unread: bool,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Issue,
    PullRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemTarget {
    pub repo: String,
    pub number: u32,
    pub kind: ItemKind,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct GitHubAccount {
    pub host: String,
    pub login: String,
    pub active: bool,
    pub state: String,
    #[serde(rename = "tokenSource", default)]
    pub token_source: String,
}

impl GitHubAccount {
    pub fn uses_environment_token(&self) -> bool {
        matches!(
            self.token_source.as_str(),
            "GH_TOKEN" | "GITHUB_TOKEN" | "GH_ENTERPRISE_TOKEN" | "GITHUB_ENTERPRISE_TOKEN"
        )
    }
}

#[derive(Deserialize)]
struct AuthStatus {
    hosts: HashMap<String, Vec<GitHubAccount>>,
}

impl Notification {
    pub fn item_target(&self) -> Option<ItemTarget> {
        let kind = match self.subject.kind.as_str() {
            "Issue" => ItemKind::Issue,
            "PullRequest" => ItemKind::PullRequest,
            _ => return None,
        };
        let url = self.subject.url.as_deref()?;
        let path = url.split(['?', '#']).next()?.trim_end_matches('/');
        let number = path.rsplit('/').next()?.parse().ok()?;

        Some(ItemTarget {
            repo: self.repository.full_name.clone(),
            number,
            kind,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotifSubject {
    pub title: String,
    #[serde(alias = "type")]
    pub kind: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotifRepo {
    pub full_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoDetail {
    #[serde(alias = "nameWithOwner")]
    pub full_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(alias = "isPrivate", default)]
    pub is_private: bool,
    #[serde(alias = "isFork", default)]
    pub is_fork: bool,
    #[serde(alias = "isArchived", default)]
    pub is_archived: bool,
    #[serde(alias = "stargazerCount", default)]
    pub star_count: u32,
    #[serde(alias = "forkCount", default)]
    pub fork_count: u32,
    #[serde(default, deserialize_with = "null_as_default")]
    pub issues: CountWrapper,
    #[serde(alias = "pullRequests", default, deserialize_with = "null_as_default")]
    pub pull_requests: CountWrapper,
    #[serde(alias = "primaryLanguage")]
    pub primary_language: Option<LangWrapper>,
    #[serde(alias = "licenseInfo")]
    pub license_info: Option<LicenseWrapper>,
    #[serde(alias = "defaultBranchRef")]
    pub default_branch_ref: Option<BranchWrapper>,
    #[serde(
        alias = "repositoryTopics",
        default,
        deserialize_with = "null_as_default"
    )]
    pub topics: Vec<TopicWrapper>,
    #[serde(alias = "createdAt")]
    pub created_at: Option<String>,
    #[serde(alias = "updatedAt")]
    pub updated_at: Option<String>,
    #[serde(alias = "homepageUrl")]
    pub homepage_url: Option<String>,
    #[serde(skip)]
    pub readme: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CountWrapper {
    #[serde(alias = "totalCount", default)]
    pub total_count: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LangWrapper {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LicenseWrapper {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BranchWrapper {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TopicWrapper {
    pub name: String,
}

// --- API functions ---

pub fn list_accounts() -> Result<Vec<GitHubAccount>> {
    let host = env::var("GH_HOST")
        .ok()
        .filter(|host| !host.trim().is_empty())
        .unwrap_or_else(|| "github.com".to_string());
    let output = run(&["auth", "status", "--hostname", &host, "--json", "hosts"])?;
    parse_accounts(&output)
}

fn parse_accounts(output: &str) -> Result<Vec<GitHubAccount>> {
    let status: AuthStatus = serde_json::from_str(output)?;
    let mut accounts: Vec<_> = status.hosts.into_values().flatten().collect();
    accounts.sort_by(|left, right| {
        left.host
            .cmp(&right.host)
            .then_with(|| right.active.cmp(&left.active))
            .then_with(|| left.login.to_lowercase().cmp(&right.login.to_lowercase()))
    });
    Ok(accounts)
}

pub fn switch_account(host: &str, login: &str) -> Result<()> {
    let token_variables: &[&str] = if host == "github.com" || host.ends_with(".ghe.com") {
        &["GH_TOKEN", "GITHUB_TOKEN"]
    } else {
        &["GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"]
    };
    if let Some(variable) = token_variables
        .iter()
        .find(|variable| env::var_os(variable).is_some_and(|value| !value.is_empty()))
    {
        anyhow::bail!(
            "{variable} overrides stored gh accounts; unset it before switching accounts"
        );
    }
    run(&["auth", "switch", "--hostname", host, "--user", login])?;
    Ok(())
}

pub fn list_repos(limit: u32) -> Result<Vec<Repo>> {
    let out = run(&[
        "repo",
        "list",
        "--json",
        "name,nameWithOwner,description,updatedAt,isPrivate,stargazerCount",
        "--limit",
        &limit.to_string(),
    ])?;
    Ok(serde_json::from_str(&out)?)
}

pub fn list_starred(limit: u32) -> Result<Vec<Repo>> {
    let out = run(&["api", &format!("user/starred?per_page={limit}")])?;
    #[derive(Deserialize)]
    struct Star {
        full_name: String,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        private: bool,
        #[serde(default)]
        stargazers_count: u32,
        #[serde(default)]
        updated_at: Option<String>,
    }
    let starred: Vec<Star> = serde_json::from_str(&out)?;
    Ok(starred
        .into_iter()
        .map(|s| Repo {
            full_name: s.full_name,
            description: s.description,
            is_private: s.private,
            star_count: s.stargazers_count,
            updated_at: s.updated_at,
        })
        .collect())
}

pub fn list_orgs() -> Result<Vec<String>> {
    let out = run(&["api", "user/orgs", "--jq", ".[].login"])?;
    Ok(out
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

pub fn list_org_repos(org: &str, limit: u32) -> Result<Vec<Repo>> {
    let out = run(&[
        "repo",
        "list",
        org,
        "--json",
        "name,nameWithOwner,description,updatedAt,isPrivate,stargazerCount",
        "--limit",
        &limit.to_string(),
    ])?;
    Ok(serde_json::from_str(&out)?)
}

pub fn list_issues(repo: &str, limit: u32) -> Result<Vec<Issue>> {
    let out = run(&[
        "issue",
        "list",
        "-R",
        repo,
        "--json",
        "number,title,state,author,labels,updatedAt",
        "--limit",
        &limit.to_string(),
    ])?;
    Ok(serde_json::from_str(&out)?)
}

pub fn list_prs(repo: &str, limit: u32) -> Result<Vec<PR>> {
    let out = run(&[
        "pr",
        "list",
        "-R",
        repo,
        "--json",
        "number,title,state,author,isDraft,updatedAt,statusCheckRollup",
        "--limit",
        &limit.to_string(),
    ])?;
    Ok(serde_json::from_str(&out)?)
}

pub fn view_issue(repo: &str, number: u32) -> Result<IssueDetail> {
    let out = run(&[
        "issue",
        "view",
        &number.to_string(),
        "-R",
        repo,
        "--json",
        "number,title,state,body,author,labels,comments",
    ])?;
    Ok(serde_json::from_str(&out)?)
}

pub fn view_pr(repo: &str, number: u32) -> Result<IssueDetail> {
    let out = run(&[
        "pr",
        "view",
        &number.to_string(),
        "-R",
        repo,
        "--json",
        "number,title,state,body,author,labels,comments",
    ])?;
    Ok(serde_json::from_str(&out)?)
}

pub fn pr_diff(repo: &str, number: u32) -> Result<String> {
    run(&["pr", "diff", &number.to_string(), "-R", repo])
}

pub fn view_repo(repo: &str) -> Result<RepoDetail> {
    let out = run(&[
        "repo",
        "view",
        repo,
        "--json",
        "name,nameWithOwner,description,homepageUrl,isPrivate,isFork,isArchived,\
         stargazerCount,forkCount,issues,pullRequests,primaryLanguage,licenseInfo,\
         defaultBranchRef,repositoryTopics,createdAt,updatedAt",
    ])?;
    let mut detail: RepoDetail = serde_json::from_str(&out)?;
    if let Ok(readme) = run(&["repo", "view", repo]) {
        detail.readme = Some(readme);
    }
    Ok(detail)
}

pub fn fetch_readme(repo: &str) -> Result<String> {
    let out = run(&[
        "api",
        &format!("repos/{repo}/readme"),
        "--header",
        "Accept: application/vnd.github.raw+json",
    ])?;
    Ok(out)
}

pub fn list_notifications() -> Result<Vec<Notification>> {
    let out = run(&["api", "notifications", "--cache", "30s"])?;
    Ok(serde_json::from_str(&out)?)
}

pub fn mark_notification_read(thread_id: &str) -> Result<()> {
    run(&[
        "api",
        "--method",
        "PATCH",
        &format!("notifications/threads/{thread_id}"),
    ])?;
    Ok(())
}

pub fn search_repos(query: &str, limit: u32) -> Result<Vec<Repo>> {
    let out = run(&[
        "search",
        "repos",
        query,
        "--json",
        "fullName,description,isPrivate,stargazersCount,updatedAt",
        "--limit",
        &limit.to_string(),
    ])?;
    Ok(serde_json::from_str(&out)?)
}

pub fn clone_repo(repo: &str, target_dir: &str) -> Result<()> {
    let name = repo.split('/').next_back().unwrap_or(repo);
    let dest = format!("{target_dir}/{name}");
    if std::path::Path::new(&dest).exists() {
        anyhow::bail!("{dest} already exists");
    }
    std::fs::create_dir_all(target_dir)?;
    run(&["repo", "clone", repo, &dest])?;
    Ok(())
}

pub fn current_repo() -> Option<String> {
    run(&[
        "repo",
        "view",
        "--json",
        "nameWithOwner",
        "-q",
        ".nameWithOwner",
    ])
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

pub fn open_in_browser(url: &str) -> Result<()> {
    Command::new("open").arg(url).spawn()?;
    Ok(())
}

pub struct UserList {
    pub name: String,
    pub repos: Vec<Repo>,
}

pub fn list_user_lists() -> Result<Vec<UserList>> {
    let query = r#"{ viewer { lists(first: 20) { nodes { name items(first: 50) { nodes { ... on Repository { nameWithOwner description stargazerCount isPrivate updatedAt } } } } } } }"#;
    let out = run(&["api", "graphql", "-f", &format!("query={query}")])?;
    #[derive(Deserialize)]
    struct RepoNode {
        #[serde(alias = "nameWithOwner")]
        name_with_owner: Option<String>,
        description: Option<String>,
        #[serde(alias = "stargazerCount", default)]
        stargazer_count: u32,
        #[serde(alias = "isPrivate", default)]
        is_private: bool,
        #[serde(alias = "updatedAt")]
        updated_at: Option<String>,
    }
    #[derive(Deserialize)]
    struct Items {
        nodes: Vec<RepoNode>,
    }
    #[derive(Deserialize)]
    struct ListNode {
        name: String,
        items: Items,
    }
    #[derive(Deserialize)]
    struct Nodes {
        nodes: Vec<ListNode>,
    }
    #[derive(Deserialize)]
    struct Viewer {
        lists: Nodes,
    }
    #[derive(Deserialize)]
    struct Data {
        viewer: Viewer,
    }
    #[derive(Deserialize)]
    struct Resp {
        data: Data,
    }
    let resp: Resp = serde_json::from_str(&out)?;
    Ok(resp
        .data
        .viewer
        .lists
        .nodes
        .into_iter()
        .map(|list| UserList {
            name: list.name,
            repos: list
                .items
                .nodes
                .into_iter()
                .filter_map(|n| {
                    Some(Repo {
                        full_name: n.name_with_owner?,
                        description: n.description,
                        is_private: n.is_private,
                        star_count: n.stargazer_count,
                        updated_at: n.updated_at,
                    })
                })
                .collect(),
        })
        .collect())
}

pub fn open_repo(repo: &str) {
    let _ = open_in_browser(&format!("https://github.com/{repo}"));
}

pub fn open_issue(repo: &str, number: u32) {
    let _ = open_in_browser(&format!("https://github.com/{repo}/issues/{number}"));
}

pub fn open_pr(repo: &str, number: u32) {
    let _ = open_in_browser(&format!("https://github.com/{repo}/pull/{number}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_errors_do_not_expose_graphql_queries() {
        let label = command_label(&["api", "graphql", "-f", "query={ viewer { login } }"]);

        assert_eq!(label, "gh api graphql");
        assert!(!label.contains("query="));
    }

    fn notification(kind: &str, url: Option<&str>) -> Notification {
        Notification {
            id: "synthetic-thread".into(),
            reason: "mention".into(),
            subject: NotifSubject {
                title: "Synthetic notification".into(),
                kind: kind.into(),
                url: url.map(String::from),
            },
            repository: NotifRepo {
                full_name: "example/synthetic-repo".into(),
            },
            unread: true,
            updated_at: None,
        }
    }

    #[test]
    fn resolves_issue_notification_target() {
        let target = notification(
            "Issue",
            Some("https://api.github.com/repos/example/synthetic-repo/issues/42"),
        )
        .item_target()
        .expect("issue notification should resolve");

        assert_eq!(target.repo, "example/synthetic-repo");
        assert_eq!(target.number, 42);
        assert_eq!(target.kind, ItemKind::Issue);
    }

    #[test]
    fn resolves_pull_request_notification_target_with_query() {
        let target = notification(
            "PullRequest",
            Some("https://api.github.com/repos/example/synthetic-repo/pulls/7?source=test"),
        )
        .item_target()
        .expect("pull request notification should resolve");

        assert_eq!(target.number, 7);
        assert_eq!(target.kind, ItemKind::PullRequest);
    }

    #[test]
    fn ignores_unsupported_or_malformed_notifications() {
        assert!(
            notification("Release", Some("https://api.github.com/releases/1"))
                .item_target()
                .is_none()
        );
        assert!(
            notification("Issue", Some("https://api.github.com/issues/not-a-number"))
                .item_target()
                .is_none()
        );
        assert!(notification("Issue", None).item_target().is_none());
    }

    #[test]
    fn parses_and_orders_accounts_without_token_fields() {
        let accounts = parse_accounts(
            r#"{
                "hosts": {
                    "github.example.invalid": [
                        {"host":"github.example.invalid","login":"enterprise-example","active":true,"state":"Logged in"}
                    ],
                    "github.com": [
                        {"host":"github.com","login":"other-example","active":false,"state":"Logged in"},
                        {"host":"github.com","login":"active-example","active":true,"state":"Logged in"}
                    ]
                }
            }"#,
        )
        .expect("synthetic auth status should parse");

        assert_eq!(accounts.len(), 3);
        assert_eq!(accounts[0].host, "github.com");
        assert_eq!(accounts[0].login, "active-example");
        assert!(accounts[0].active);
        assert_eq!(accounts[2].host, "github.example.invalid");
    }
}
