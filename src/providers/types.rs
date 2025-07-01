use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Repository provider types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProviderType {
    Bitbucket,
    GitHub,
    GitLab,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Bitbucket => write!(f, "Bitbucket"),
            ProviderType::GitHub => write!(f, "GitHub"),
            ProviderType::GitLab => write!(f, "GitLab"),
        }
    }
}

/// Provider health status
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderHealth {
    pub is_healthy: bool,
    pub can_authenticate: bool,
    pub can_create_prs: bool,
    pub message: Option<String>,
}

/// Repository information
#[derive(Debug, Clone)]
pub struct RepositoryInfo {
    pub provider: ProviderType,
    pub url: String,
    pub project: String,    // namespace/owner/project key
    pub repository: String, // repo name/slug
    pub default_branch: String,
}

/// Pull request information
#[derive(Debug, Clone)]
pub struct PullRequest {
    pub id: String,
    pub title: String,
    pub description: String,
    pub source_branch: String,
    pub target_branch: String,
    pub author: String,
    pub status: PullRequestStatus,
    pub web_url: String,
    pub reviewers: Vec<Reviewer>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Pull request status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PullRequestStatus {
    Open,
    Merged,
    Declined,
    Superseded,
}

/// Reviewer information
#[derive(Debug, Clone)]
pub struct Reviewer {
    pub username: String,
    pub display_name: String,
    pub approved: bool,
}

/// Merge strategy options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MergeStrategy {
    Merge,
    Squash,
    FastForward,
    SquashFastForward,
}

impl std::fmt::Display for MergeStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MergeStrategy::Merge => write!(f, "merge"),
            MergeStrategy::Squash => write!(f, "squash"),
            MergeStrategy::FastForward => write!(f, "fast-forward"),
            MergeStrategy::SquashFastForward => write!(f, "squash-fast-forward"),
        }
    }
}

/// Request to create a pull request
#[derive(Debug, Clone)]
pub struct CreatePullRequestRequest {
    pub title: String,
    pub description: String,
    pub source_branch: String,
    pub target_branch: String,
    pub reviewers: Vec<String>,
}

/// Request to update a pull request
#[derive(Debug, Clone)]
pub struct UpdatePullRequestRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub reviewers: Option<Vec<String>>,
}

/// Build status information
#[derive(Debug, Clone, PartialEq)]
pub enum BuildStatus {
    Success,
    Failed,
    InProgress,
    NotStarted,
    Unknown,
}

impl std::fmt::Display for BuildStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildStatus::Success => write!(f, "success"),
            BuildStatus::Failed => write!(f, "failed"),
            BuildStatus::InProgress => write!(f, "in-progress"),
            BuildStatus::NotStarted => write!(f, "not-started"),
            BuildStatus::Unknown => write!(f, "unknown"),
        }
    }
}

/// Build information with detailed context
#[derive(Debug, Clone)]
pub struct BuildInfo {
    pub status: BuildStatus,
    pub url: Option<String>,
    pub description: Option<String>,
    pub context: Option<String>,
}

/// Merge operation result
#[derive(Debug, Clone)]
pub struct MergeResult {
    pub merged_commit_hash: String,
    pub message: Option<String>,
}
