use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::time::Duration;

/// Max retries for GitHub API calls (rate limit / transient errors).
const GITHUB_API_MAX_RETRIES: u32 = 3;
/// Initial backoff duration; doubles each retry.
const GITHUB_API_INITIAL_BACKOFF_MS: u64 = 1000;

/// Run an async closure with retry and exponential backoff. Retries on 429 (rate limit),
/// 503 (unavailable), and transient reqwest errors.
async fn with_retry<F, Fut, T>(mut f: F) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let mut backoff_ms = GITHUB_API_INITIAL_BACKOFF_MS;
    for attempt in 0..=GITHUB_API_MAX_RETRIES {
        match f().await {
            Ok(t) => return Ok(t),
            Err(e) => {
                let retryable = e.contains("429")
                    || e.contains("503")
                    || e.contains("Failed to query")
                    || e.contains("Failed to create")
                    || e.contains("Failed to update");
                if retryable && attempt < GITHUB_API_MAX_RETRIES {
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = (backoff_ms * 2).min(30_000);
                } else {
                    return Err(e);
                }
            }
        }
    }
    unreachable!()
}

/// Repository information extracted from URL
pub struct RepoInfo {
    pub owner: String,
    pub name: String,
}

/// Extract owner and repository name from a git URL
/// Supports:
/// - https://github.com/owner/repo.git
/// - https://github.com/owner/repo
/// - git@github.com:owner/repo.git
/// - git@github.com:owner/repo
pub fn extract_repo_info(url: &str) -> Result<RepoInfo, String> {
    // Remove .git suffix if present
    let url = url.trim_end_matches(".git");

    // Handle SSH URLs (git@github.com:owner/repo)
    if url.starts_with("git@") {
        let parts: Vec<&str> = url.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid SSH URL format: {}", url));
        }
        let repo_part = parts[1];
        let repo_parts: Vec<&str> = repo_part.split('/').collect();
        if repo_parts.len() < 2 {
            return Err(format!("Invalid repository path in SSH URL: {}", url));
        }
        let owner = repo_parts[repo_parts.len() - 2].to_string();
        let name = repo_parts[repo_parts.len() - 1].to_string();
        return Ok(RepoInfo { owner, name });
    }

    // Handle HTTPS URLs (https://github.com/owner/repo)
    if url.contains("github.com") {
        let parts: Vec<&str> = url.split("github.com").collect();
        if parts.len() != 2 {
            return Err(format!("Invalid GitHub URL format: {}", url));
        }
        let path = parts[1].trim_start_matches('/').trim_end_matches('/');
        let path_parts: Vec<&str> = path.split('/').collect();
        if path_parts.len() < 2 {
            return Err(format!("Invalid repository path in GitHub URL: {}", url));
        }
        let owner = path_parts[0].to_string();
        let name = path_parts[1].to_string();
        return Ok(RepoInfo { owner, name });
    }

    Err(format!("Unsupported repository URL format: {}", url))
}

/// GitHub PR response
#[derive(Debug, Serialize, Deserialize)]
struct PullRequest {
    number: u64,
    html_url: String,
    title: String,
    body: Option<String>,
    head: BranchRef,
    base: BranchRef,
    state: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BranchRef {
    #[serde(rename = "ref")]
    ref_name: String,
}

/// Create PR request payload
#[derive(Debug, Serialize)]
struct CreatePRRequest {
    title: String,
    body: String,
    head: String,
    base: String,
}

/// Update PR request payload
#[derive(Debug, Serialize)]
struct UpdatePRRequest {
    title: Option<String>,
    body: Option<String>,
}

/// Find existing PR for a branch
async fn find_existing_pr(
    token: &str,
    owner: &str,
    repo: &str,
    branch: &str,
) -> Result<Option<PullRequest>, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/{}/pulls?head={}:{}&state=open",
        owner, repo, owner, branch
    );

    let response = client
        .get(&url)
        .header("Authorization", format!("token {}", token))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "agent-smith")
        .send()
        .await
        .map_err(|e| format!("Failed to query GitHub API: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("GitHub API error ({}): {}", status, error_text));
    }

    let prs: Vec<PullRequest> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub API response: {}", e))?;

    // Find PR with matching head branch
    for pr in prs {
        if pr.head.ref_name == branch {
            return Ok(Some(pr));
        }
    }

    Ok(None)
}

/// Create a new pull request
async fn create_pr(
    token: &str,
    owner: &str,
    repo: &str,
    branch: &str,
    base: &str,
    title: &str,
    body: &str,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let url = format!("https://api.github.com/repos/{}/{}/pulls", owner, repo);

    let payload = CreatePRRequest {
        title: title.to_string(),
        body: body.to_string(),
        head: branch.to_string(),
        base: base.to_string(),
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("token {}", token))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "agent-smith")
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to create PR: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Failed to create PR ({}): {}", status, error_text));
    }

    let pr: PullRequest = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse PR response: {}", e))?;

    Ok(pr.html_url)
}

/// Update an existing pull request
async fn update_pr(
    token: &str,
    owner: &str,
    repo: &str,
    pr_number: u64,
    title: Option<&str>,
    body: Option<&str>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/{}/pulls/{}",
        owner, repo, pr_number
    );

    let payload = UpdatePRRequest {
        title: title.map(|s| s.to_string()),
        body: body.map(|s| s.to_string()),
    };

    let response = client
        .patch(&url)
        .header("Authorization", format!("token {}", token))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "agent-smith")
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to update PR: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Failed to update PR ({}): {}", status, error_text));
    }

    let pr: PullRequest = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse PR response: {}", e))?;

    Ok(pr.html_url)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

impl Default for MergeMethod {
    fn default() -> Self {
        MergeMethod::Merge
    }
}

impl FromStr for MergeMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "merge" => Ok(MergeMethod::Merge),
            "squash" => Ok(MergeMethod::Squash),
            "rebase" => Ok(MergeMethod::Rebase),
            _ => Err(format!(
                "Invalid merge method '{}'. Valid options: merge, squash, rebase",
                s
            )),
        }
    }
}

/// Merge a pull request
async fn merge_pr(
    token: &str,
    owner: &str,
    repo: &str,
    pr_number: u64,
    commit_title: &str,
    commit_message: &str,
    merge_method: MergeMethod,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/{}/pulls/{}/merge",
        owner, repo, pr_number
    );

    let method_str = match merge_method {
        MergeMethod::Merge => "merge",
        MergeMethod::Squash => "squash",
        MergeMethod::Rebase => "rebase",
    };

    #[derive(Serialize)]
    struct MergeRequest {
        commit_title: String,
        commit_message: String,
        merge_method: String,
    }

    let payload = MergeRequest {
        commit_title: commit_title.to_string(),
        commit_message: commit_message.to_string(),
        merge_method: method_str.to_string(),
    };

    let response = client
        .put(&url)
        .header("Authorization", format!("token {}", token))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "agent-smith")
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to merge PR: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Failed to merge PR ({}): {}", status, error_text));
    }

    #[derive(Deserialize)]
    struct MergeResponse {
        sha: String,
        #[allow(dead_code)]
        merged: bool,
        message: String,
    }

    let result: MergeResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse merge response: {}", e))?;

    Ok(format!("{} ({})", result.sha, result.message))
}

/// Get PR number by branch name
async fn get_pr_by_branch(
    token: &str,
    owner: &str,
    repo: &str,
    branch: &str,
) -> Result<Option<u64>, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/{}/pulls?head={}:{}&state=open",
        owner, repo, owner, branch
    );

    let response = client
        .get(&url)
        .header("Authorization", format!("token {}", token))
        .header("Accept", "application/vnd.github.v3+json")
        .header("User-Agent", "agent-smith")
        .send()
        .await
        .map_err(|e| format!("Failed to query GitHub API: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("GitHub API error ({}): {}", status, error_text));
    }

    #[derive(Deserialize)]
    struct PullRequest {
        number: u64,
    }

    let prs: Vec<PullRequest> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub API response: {}", e))?;

    Ok(prs.into_iter().next().map(|pr| pr.number))
}

/// Merge a branch into its base branch via PR
/// Finds an open PR for the branch and merges it
pub async fn merge_branch(
    token: &str,
    owner: &str,
    repo: &str,
    branch: &str,
    commit_message: Option<&str>,
    merge_method: MergeMethod,
) -> Result<String, String> {
    let pr_number =
        with_retry(|| async move { get_pr_by_branch(token, owner, repo, branch).await })
            .await?
            .ok_or_else(|| format!("No open PR found for branch '{}'", branch))?;

    let default_message = format!("Merge branch '{}' via Agent Smith", branch);
    let message = commit_message.unwrap_or(&default_message);

    with_retry(|| async move {
        merge_pr(
            token,
            owner,
            repo,
            pr_number,
            &format!("Merge branch '{}'", branch),
            message,
            merge_method,
        )
        .await
    })
    .await
}

/// Create or update a pull request
/// Only creates one PR per branch (updates existing if found).
/// Uses retry with backoff for rate limits and transient errors.
pub async fn create_or_update_pr(
    token: &str,
    owner: &str,
    repo: &str,
    branch: &str,
    base: &str,
    title: &str,
) -> Result<String, String> {
    let existing =
        with_retry(|| async move { find_existing_pr(token, owner, repo, branch).await }).await;

    match existing {
        Ok(Some(existing_pr)) => {
            println!(
                "  Found existing PR #{} for branch '{}', updating...",
                existing_pr.number, branch
            );
            with_retry(|| async move {
                update_pr(token, owner, repo, existing_pr.number, Some(title), None).await
            })
            .await
        }
        Ok(None) => {
            println!("  Creating new pull request for branch '{}'...", branch);
            let body = format!(
                "Automated PR created by Agent Smith for branch `{}`",
                branch
            );
            with_retry(|| {
                let b = body.clone();
                async move { create_pr(token, owner, repo, branch, base, title, &b).await }
            })
            .await
        }
        Err(e) => {
            println!("  Could not check for existing PRs: {}", e);
            println!("  Attempting to create new pull request...");
            let body = format!(
                "Automated PR created by Agent Smith for branch `{}`",
                branch
            );
            with_retry(|| {
                let b = body.clone();
                async move { create_pr(token, owner, repo, branch, base, title, &b).await }
            })
            .await
        }
    }
}
