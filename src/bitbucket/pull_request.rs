use crate::errors::Result;
use crate::bitbucket::client::BitbucketClient;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tracing::info;

/// Pull request manager for Bitbucket operations
pub struct PullRequestManager {
    client: BitbucketClient,
}

impl PullRequestManager {
    /// Create a new pull request manager
    pub fn new(client: BitbucketClient) -> Self {
        Self { client }
    }

    /// Create a new pull request
    pub async fn create_pull_request(&self, request: CreatePullRequestRequest) -> Result<PullRequest> {
        info!("Creating pull request: {} -> {}", request.from_ref.id, request.to_ref.id);
        
        let pr: PullRequest = self.client.post("pull-requests", &request).await?;
        
        info!("Created pull request #{}: {}", pr.id, pr.title);
        Ok(pr)
    }

    /// Get a pull request by ID
    pub async fn get_pull_request(&self, pr_id: u64) -> Result<PullRequest> {
        self.client.get(&format!("pull-requests/{}", pr_id)).await
    }

    /// List pull requests with optional filters
    pub async fn list_pull_requests(&self, state: Option<PullRequestState>) -> Result<PullRequestPage> {
        let mut path = "pull-requests".to_string();
        
        if let Some(state) = state {
            path.push_str(&format!("?state={}", state.as_str()));
        }
        
        self.client.get(&path).await
    }
}

/// Request to create a new pull request
#[derive(Debug, Serialize)]
pub struct CreatePullRequestRequest {
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "fromRef")]
    pub from_ref: PullRequestRef,
    #[serde(rename = "toRef")]
    pub to_ref: PullRequestRef,
}

/// Pull request data structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PullRequest {
    pub id: u64,
    pub version: u64,
    pub title: String,
    pub description: Option<String>,
    pub state: PullRequestState,
    pub open: bool,
    pub closed: bool,
    #[serde(rename = "createdDate")]
    pub created_date: u64,
    #[serde(rename = "updatedDate")]
    pub updated_date: u64,
    #[serde(rename = "fromRef")]
    pub from_ref: PullRequestRef,
    #[serde(rename = "toRef")]
    pub to_ref: PullRequestRef,
    pub locked: bool,
    pub author: Participant,
    pub links: PullRequestLinks,
}

/// Pull request reference (branch information)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PullRequestRef {
    pub id: String,
    #[serde(rename = "displayId")]
    pub display_id: String,
    #[serde(rename = "latestCommit")]
    pub latest_commit: String,
    pub repository: Repository,
}

/// Repository information in pull request context
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Repository {
    pub id: u64,
    pub name: String,
    pub slug: String,
    #[serde(rename = "scmId")]
    pub scm_id: String,
    pub state: String,
    #[serde(rename = "statusMessage")]
    pub status_message: String,
    pub forkable: bool,
    pub project: Project,
    pub public: bool,
}

/// Project information in pull request context
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Project {
    pub id: u64,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub public: bool,
    #[serde(rename = "type")]
    pub project_type: String,
}

/// Pull request links
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PullRequestLinks {
    #[serde(rename = "self")]
    pub self_link: Vec<SelfLink>,
}

/// Self link
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SelfLink {
    pub href: String,
}

/// Pull request participant
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Participant {
    pub user: User,
    pub role: ParticipantRole,
    pub approved: bool,
    pub status: ParticipantStatus,
}

/// User information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct User {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "emailAddress")]
    pub email_address: String,
    pub active: bool,
    pub slug: String,
}

/// Pull request state
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PullRequestState {
    Open,
    Merged,
    Declined,
}

impl PullRequestState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Merged => "MERGED", 
            Self::Declined => "DECLINED",
        }
    }
}

/// Participant role
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ParticipantRole {
    Author,
    Reviewer,
    Participant,
}

/// Participant status
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ParticipantStatus {
    Approved,
    Unapproved,
    #[serde(rename = "NEEDS_WORK")]
    NeedsWork,
}

/// Paginated pull request results
#[derive(Debug, Deserialize)]
pub struct PullRequestPage {
    pub size: u32,
    pub limit: u32,
    #[serde(rename = "isLastPage")]
    pub is_last_page: bool,
    pub values: Vec<PullRequest>,
    pub start: u32,
    #[serde(rename = "nextPageStart")]
    pub next_page_start: Option<u32>,
}

impl PullRequest {
    /// Get the pull request URL
    pub fn web_url(&self) -> Option<String> {
        self.links.self_link.first().map(|link| link.href.clone())
    }

    /// Check if the pull request is still open
    pub fn is_open(&self) -> bool {
        self.state == PullRequestState::Open && self.open && !self.closed
    }

    /// Get the created date as a DateTime
    pub fn created_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.created_date as i64 / 1000, 0)
            .unwrap_or_else(|| Utc::now())
    }

    /// Get the updated date as a DateTime
    pub fn updated_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.updated_date as i64 / 1000, 0)
            .unwrap_or_else(|| Utc::now())
    }
}
