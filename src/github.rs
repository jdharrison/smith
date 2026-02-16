use serde::{Deserialize, Serialize};

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

/// Create or update a pull request
/// Only creates one PR per branch (updates existing if found)
pub async fn create_or_update_pr(
    token: &str,
    owner: &str,
    repo: &str,
    branch: &str,
    base: &str,
    title: &str,
) -> Result<String, String> {
    // Try to find existing PR for this branch
    match find_existing_pr(token, owner, repo, branch).await {
        Ok(Some(existing_pr)) => {
            // Update existing PR
            println!(
                "  Found existing PR #{} for branch '{}', updating...",
                existing_pr.number, branch
            );
            update_pr(token, owner, repo, existing_pr.number, Some(title), None).await
        }
        Ok(None) => {
            // Create new PR
            println!("  Creating new pull request for branch '{}'...", branch);
            let body = format!(
                "Automated PR created by Agent Smith for branch `{}`",
                branch
            );
            create_pr(token, owner, repo, branch, base, title, &body).await
        }
        Err(e) => {
            // If we can't find existing PRs, try to create anyway
            // (might be a permissions issue, but creation might still work)
            println!("  Could not check for existing PRs: {}", e);
            println!("  Attempting to create new pull request...");
            let body = format!(
                "Automated PR created by Agent Smith for branch `{}`",
                branch
            );
            create_pr(token, owner, repo, branch, base, title, &body).await
        }
    }
}
