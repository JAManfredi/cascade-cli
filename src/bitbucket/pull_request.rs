use crate::bitbucket::client::BitbucketClient;
use crate::errors::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
    pub async fn create_pull_request(
        &self,
        request: CreatePullRequestRequest,
    ) -> Result<PullRequest> {
        info!(
            "Creating pull request: {} -> {}",
            request.from_ref.id, request.to_ref.id
        );

        let pr: PullRequest = self.client.post("pull-requests", &request).await?;

        info!("Created pull request #{}: {}", pr.id, pr.title);
        Ok(pr)
    }

    /// Get a pull request by ID
    pub async fn get_pull_request(&self, pr_id: u64) -> Result<PullRequest> {
        self.client.get(&format!("pull-requests/{pr_id}")).await
    }

    /// List pull requests with optional filters
    pub async fn list_pull_requests(
        &self,
        state: Option<PullRequestState>,
    ) -> Result<PullRequestPage> {
        let mut path = "pull-requests".to_string();

        if let Some(state) = state {
            path.push_str(&format!("?state={}", state.as_str()));
        }

        self.client.get(&path).await
    }

    /// Update a pull request's source branch by closing the old PR and creating a new one
    /// This is needed because Bitbucket doesn't allow changing PR source branches
    pub async fn update_source_branch(
        &self,
        old_pr_id: u64,
        new_request: CreatePullRequestRequest,
        close_reason: Option<String>,
    ) -> Result<PullRequest> {
        info!(
            "Updating PR #{} source branch: {} -> {}",
            old_pr_id, old_pr_id, new_request.from_ref.display_id
        );

        // First, get the old PR to preserve information
        let old_pr = self.get_pull_request(old_pr_id).await?;

        // Close/decline the old PR with a descriptive message
        let close_message = close_reason.unwrap_or_else(|| {
            format!(
                "Superseded by updated branch: {}",
                new_request.from_ref.display_id
            )
        });

        self.decline_pull_request(old_pr_id, &close_message).await?;

        // Create new PR with the same title/description but new branch
        let new_request = CreatePullRequestRequest {
            title: format!("{} (Updated)", old_pr.title),
            description: old_pr.description.clone(),
            from_ref: new_request.from_ref,
            to_ref: new_request.to_ref,
        };

        let new_pr = self.create_pull_request(new_request).await?;

        info!("Closed PR #{} and created new PR #{}", old_pr_id, new_pr.id);
        Ok(new_pr)
    }

    /// Decline a pull request with a reason
    pub async fn decline_pull_request(&self, pr_id: u64, reason: &str) -> Result<()> {
        info!("Declining pull request #{}: {}", pr_id, reason);

        #[derive(Serialize)]
        struct DeclineRequest {
            version: u64,
            #[serde(rename = "participantStatus")]
            participant_status: String,
        }

        // First get the current PR to get its version
        let pr = self.get_pull_request(pr_id).await?;

        let decline_body = DeclineRequest {
            version: pr.version,
            participant_status: "DECLINED".to_string(),
        };

        let path = format!("pull-requests/{pr_id}/decline");

        // Use the client to make the decline request
        let _: serde_json::Value = self.client.post(&path, &decline_body).await?;

        info!("Successfully declined pull request #{}", pr_id);
        Ok(())
    }

    /// Add a comment to a pull request explaining the branch update
    pub async fn add_comment(&self, pr_id: u64, comment: &str) -> Result<()> {
        info!("Adding comment to PR #{}", pr_id);

        #[derive(Serialize)]
        struct CommentRequest {
            text: String,
        }

        let comment_body = CommentRequest {
            text: comment.to_string(),
        };

        let path = format!("pull-requests/{pr_id}/comments");
        let _: serde_json::Value = self.client.post(&path, &comment_body).await?;

        info!("Added comment to PR #{}", pr_id);
        Ok(())
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
        DateTime::from_timestamp(self.created_date as i64 / 1000, 0).unwrap_or_else(Utc::now)
    }

    /// Get the updated date as a DateTime
    pub fn updated_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.updated_date as i64 / 1000, 0).unwrap_or_else(Utc::now)
    }
}
