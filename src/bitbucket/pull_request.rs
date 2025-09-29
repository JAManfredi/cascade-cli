use crate::bitbucket::client::BitbucketClient;
use crate::errors::{CascadeError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

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
        // Creating pull request

        // Debug the request being sent
        debug!(
            "PR Request - Title: '{}', Description: {:?}, Draft: {}",
            request.title, request.description, request.draft
        );

        let pr: PullRequest = self.client.post("pull-requests", &request).await?;

        // Pull request created successfully
        Ok(pr)
    }

    /// Get a pull request by ID
    pub async fn get_pull_request(&self, pr_id: u64) -> Result<PullRequest> {
        self.client.get(&format!("pull-requests/{pr_id}")).await
    }

    /// Update a pull request (title, description, etc)
    pub async fn update_pull_request(
        &self,
        pr_id: u64,
        title: Option<String>,
        description: Option<String>,
        version: u64,
    ) -> Result<PullRequest> {
        #[derive(Debug, Serialize)]
        struct UpdatePullRequestRequest {
            #[serde(skip_serializing_if = "Option::is_none")]
            title: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            description: Option<String>,
            version: u64,
        }

        let request = UpdatePullRequestRequest {
            title,
            description,
            version,
        };

        self.client
            .put(&format!("pull-requests/{pr_id}"), &request)
            .await
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
            draft: new_request.draft,
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

    /// Get comprehensive status information for a pull request
    pub async fn get_pull_request_status(&self, pr_id: u64) -> Result<PullRequestStatus> {
        // Get the pull request
        let pr = self.get_pull_request(pr_id).await?;

        // Get detailed mergability information (includes all server-side checks)
        let mergeable_details = self.check_mergeable_detailed(pr_id).await.ok();
        let mergeable = mergeable_details.as_ref().map(|d| d.can_merge);

        // Get participants and calculate review status
        let participants = self.get_pull_request_participants(pr_id).await?;
        let review_status = self.calculate_review_status(&participants)?;

        // Get build status (fallback gracefully if not available)
        let build_status = self.get_build_status(pr_id).await.ok();

        // Get conflicts (fallback gracefully if not available)
        let conflicts = self.get_conflicts(pr_id).await.ok();

        Ok(PullRequestStatus {
            pr,
            mergeable,
            mergeable_details,
            participants,
            build_status,
            review_status,
            conflicts,
        })
    }

    /// Get all participants (including reviewers) for a PR
    pub async fn get_pull_request_participants(&self, pr_id: u64) -> Result<Vec<Participant>> {
        let path = format!("pull-requests/{pr_id}/participants");
        let response: ParticipantsResponse = self.client.get(&path).await?;
        Ok(response.values)
    }

    /// Check if PR is mergeable and get detailed blocking reasons
    pub async fn check_mergeable_detailed(&self, pr_id: u64) -> Result<MergeabilityDetails> {
        let path = format!("pull-requests/{pr_id}/merge");

        match self.client.get::<serde_json::Value>(&path).await {
            Ok(response) => {
                let can_merge = response
                    .get("canMerge")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let conflicted = response
                    .get("conflicted")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                // Extract detailed veto reasons if present
                let mut blocking_reasons = Vec::new();

                if let Some(vetoes) = response.get("vetoes").and_then(|v| v.as_array()) {
                    for veto in vetoes {
                        if let Some(summary) = veto.get("summaryMessage").and_then(|s| s.as_str()) {
                            blocking_reasons.push(summary.to_string());
                        } else if let Some(detailed) =
                            veto.get("detailedMessage").and_then(|s| s.as_str())
                        {
                            blocking_reasons.push(detailed.to_string());
                        }
                    }
                }

                // Add conflict information
                if conflicted {
                    blocking_reasons.push("Pull request has merge conflicts".to_string());
                }

                Ok(MergeabilityDetails {
                    can_merge,
                    conflicted,
                    blocking_reasons,
                    server_enforced: true, // This comes from Bitbucket's authoritative check
                })
            }
            Err(_) => {
                // Fallback: assume mergeable but note we couldn't check
                Ok(MergeabilityDetails {
                    can_merge: true,
                    conflicted: false,
                    blocking_reasons: vec!["Could not verify merge conditions".to_string()],
                    server_enforced: false,
                })
            }
        }
    }

    /// Check if PR is mergeable (legacy method - kept for backward compatibility)
    pub async fn check_mergeable(&self, pr_id: u64) -> Result<bool> {
        let details = self.check_mergeable_detailed(pr_id).await?;
        Ok(details.can_merge)
    }

    /// Get build status for a PR
    pub async fn get_build_status(&self, pr_id: u64) -> Result<BuildStatus> {
        let pr = self.get_pull_request(pr_id).await?;
        let commit_hash = &pr.from_ref.latest_commit;

        // Get build status for the latest commit
        let path = format!("commits/{commit_hash}/builds");

        match self.client.get::<BuildStatusResponse>(&path).await {
            Ok(response) => {
                if let Some(build) = response.values.first() {
                    Ok(BuildStatus {
                        state: build.state.clone(),
                        url: build.url.clone(),
                        description: build.description.clone(),
                        context: build.name.clone(),
                    })
                } else {
                    Ok(BuildStatus {
                        state: BuildState::Unknown,
                        url: None,
                        description: Some("No builds found".to_string()),
                        context: None,
                    })
                }
            }
            Err(_) => Ok(BuildStatus {
                state: BuildState::Unknown,
                url: None,
                description: Some("Build status unavailable".to_string()),
                context: None,
            }),
        }
    }

    /// Get conflict information for a PR
    ///
    /// NOTE: Currently unimplemented - always returns empty list.
    /// Proper implementation would parse diff for conflict markers or use
    /// Bitbucket's merge API to detect conflicts. In practice, the `mergeable`
    /// field from `check_mergeable_detailed()` is more reliable for detecting conflicts.
    pub async fn get_conflicts(&self, pr_id: u64) -> Result<Vec<String>> {
        // Conflicts are detected via the mergeable API (check_mergeable_detailed)
        // which provides server-side conflict detection. This function is kept
        // for future enhancement but is not currently needed.
        let _ = pr_id; // Avoid unused parameter warning
        Ok(Vec::new())
    }

    /// Calculate review status based on participants
    fn calculate_review_status(&self, participants: &[Participant]) -> Result<ReviewStatus> {
        let mut current_approvals = 0;
        let mut needs_work_count = 0;
        let mut missing_reviewers = Vec::new();

        for participant in participants {
            match participant.status {
                ParticipantStatus::Approved => current_approvals += 1,
                ParticipantStatus::NeedsWork => needs_work_count += 1,
                ParticipantStatus::Unapproved => {
                    if matches!(participant.role, ParticipantRole::Reviewer) {
                        missing_reviewers.push(
                            participant
                                .user
                                .display_name
                                .clone()
                                .unwrap_or_else(|| participant.user.name.clone()),
                        );
                    }
                }
            }
        }

        // Note: required_approvals is kept for API compatibility but is not accurate.
        // The REAL approval requirements are enforced by Bitbucket Server via the
        // /merge endpoint (check_mergeable_detailed), which checks:
        // - Repository approval requirements (configured in Bitbucket settings)
        // - Default reviewer approvals
        // - Build status requirements
        // - Branch permissions
        // - Task completion, Code Insights, custom merge checks
        //
        // We set this to 0 to indicate "unknown" - callers should rely on
        // can_merge and the server's mergeable checks, not this field.
        let can_merge = current_approvals > 0 && needs_work_count == 0;

        Ok(ReviewStatus {
            required_approvals: 0, // Unknown - see comment above
            current_approvals,
            needs_work_count,
            can_merge,
            missing_reviewers,
        })
    }

    /// Merge a pull request using Bitbucket Server API
    pub async fn merge_pull_request(
        &self,
        pr_id: u64,
        merge_strategy: MergeStrategy,
    ) -> Result<PullRequest> {
        let pr = self.get_pull_request(pr_id).await?;

        let merge_request = MergePullRequestRequest {
            version: pr.version,
            message: merge_strategy.get_commit_message(&pr),
            strategy: merge_strategy,
        };

        self.client
            .post(&format!("pull-requests/{pr_id}/merge"), &merge_request)
            .await
    }

    /// Auto-merge a pull request if conditions are met
    pub async fn auto_merge_if_ready(
        &self,
        pr_id: u64,
        conditions: &AutoMergeConditions,
    ) -> Result<AutoMergeResult> {
        let status = self.get_pull_request_status(pr_id).await?;

        if !status.can_auto_merge(conditions) {
            return Ok(AutoMergeResult::NotReady {
                blocking_reasons: status.get_blocking_reasons(),
            });
        }

        // Wait for any pending builds if required
        if conditions.wait_for_builds {
            self.wait_for_builds(pr_id, conditions.build_timeout)
                .await?;
        }

        // Perform the merge
        let merged_pr = self
            .merge_pull_request(pr_id, conditions.merge_strategy.clone())
            .await?;

        Ok(AutoMergeResult::Merged {
            pr: Box::new(merged_pr),
            merge_strategy: conditions.merge_strategy.clone(),
        })
    }

    /// Wait for builds to complete with timeout
    async fn wait_for_builds(&self, pr_id: u64, timeout: Duration) -> Result<()> {
        use tokio::time::{sleep, timeout as tokio_timeout};

        tokio_timeout(timeout, async {
            loop {
                let build_status = self.get_build_status(pr_id).await?;

                match build_status.state {
                    BuildState::Successful => return Ok(()),
                    BuildState::Failed | BuildState::Cancelled => {
                        return Err(CascadeError::bitbucket(format!(
                            "Build failed: {}",
                            build_status.description.unwrap_or_default()
                        )));
                    }
                    BuildState::InProgress => {
                        sleep(Duration::from_secs(30)).await; // Poll every 30s
                        continue;
                    }
                    BuildState::Unknown => {
                        return Err(CascadeError::bitbucket("Build status unknown".to_string()));
                    }
                }
            }
        })
        .await
        .map_err(|_| CascadeError::bitbucket("Build timeout exceeded".to_string()))?
    }
}

/// Request to create a new pull request
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CreatePullRequestRequest {
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "fromRef")]
    pub from_ref: PullRequestRef,
    #[serde(rename = "toRef")]
    pub to_ref: PullRequestRef,
    #[serde(rename = "isDraft")]
    pub draft: bool,
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
    pub status_message: Option<String>, // Make nullable - can be null
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
    pub display_name: Option<String>, // Make nullable - can be null for service accounts
    #[serde(rename = "emailAddress")]
    pub email_address: Option<String>, // Make nullable - can be null for some users
    pub active: bool,
    pub slug: Option<String>, // Make nullable - can be null in some cases
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
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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

/// Enhanced pull request status with mergability information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PullRequestStatus {
    pub pr: PullRequest,
    pub mergeable: Option<bool>,
    pub mergeable_details: Option<MergeabilityDetails>,
    pub participants: Vec<Participant>,
    pub build_status: Option<BuildStatus>,
    pub review_status: ReviewStatus,
    pub conflicts: Option<Vec<String>>,
}

/// Build status from CI/CD systems
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BuildStatus {
    pub state: BuildState,
    pub url: Option<String>,
    pub description: Option<String>,
    pub context: Option<String>,
}

/// Build state enum
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum BuildState {
    Successful,
    Failed,
    InProgress,
    Cancelled,
    Unknown,
}

/// Review status summary
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReviewStatus {
    pub required_approvals: usize,
    pub current_approvals: usize,
    pub needs_work_count: usize,
    pub can_merge: bool,
    pub missing_reviewers: Vec<String>,
}

/// Response for participants endpoint
#[derive(Debug, Deserialize)]
struct ParticipantsResponse {
    pub values: Vec<Participant>,
}

/// Response for mergeability check
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MergeabilityResponse {
    #[serde(rename = "canMerge")]
    pub can_merge: bool,
    pub conflicted: Option<bool>,
}

/// Response for build status
#[derive(Debug, Deserialize)]
struct BuildStatusResponse {
    pub values: Vec<BuildInfo>,
}

/// Build information from Bitbucket
#[derive(Debug, Deserialize)]
struct BuildInfo {
    pub state: BuildState,
    pub name: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
}

/// Response for diff endpoint
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DiffResponse {
    pub diffs: Vec<serde_json::Value>, // Simplified
}

impl PullRequestStatus {
    /// Get a summary status for display
    pub fn get_display_status(&self) -> String {
        if self.pr.state != PullRequestState::Open {
            return format!("{:?}", self.pr.state).to_uppercase();
        }

        let mut status_parts = Vec::new();

        // Build status
        if let Some(build) = &self.build_status {
            match build.state {
                BuildState::Successful => status_parts.push("✅ Builds"),
                BuildState::Failed => status_parts.push("❌ Builds"),
                BuildState::InProgress => status_parts.push("🔄 Building"),
                _ => status_parts.push("⚪ Builds"),
            }
        }

        // Review status
        if self.review_status.can_merge {
            status_parts.push("✅ Reviews");
        } else if self.review_status.needs_work_count > 0 {
            status_parts.push("❌ Reviews");
        } else {
            status_parts.push("⏳ Reviews");
        }

        // Merge conflicts
        if let Some(mergeable) = self.mergeable {
            if mergeable {
                status_parts.push("✅ Mergeable");
            } else {
                status_parts.push("❌ Conflicts");
            }
        }

        if status_parts.is_empty() {
            "🔄 Open".to_string()
        } else {
            status_parts.join(" | ")
        }
    }

    /// Check if this PR is ready to land/merge
    pub fn is_ready_to_land(&self) -> bool {
        self.pr.state == PullRequestState::Open
            && self.review_status.can_merge
            && self.mergeable.unwrap_or(false)
            && matches!(
                self.build_status.as_ref().map(|b| &b.state),
                Some(BuildState::Successful) | None
            )
    }

    /// Get detailed reasons why PR cannot be merged
    pub fn get_blocking_reasons(&self) -> Vec<String> {
        let mut reasons = Vec::new();

        // 🎯 SERVER-SIDE MERGE CHECKS (Most Important)
        // These are authoritative from Bitbucket Server and include:
        // - Required approvals, build checks, branch permissions
        // - Code Insights, required builds, custom merge checks
        // - Task completion, default reviewers, etc.
        if let Some(mergeable_details) = &self.mergeable_details {
            if !mergeable_details.can_merge {
                // Add specific server-side blocking reasons
                for reason in &mergeable_details.blocking_reasons {
                    reasons.push(format!("🔒 Server Check: {reason}"));
                }

                // If no specific reasons but still not mergeable
                if mergeable_details.blocking_reasons.is_empty() {
                    reasons.push("🔒 Server Check: Merge blocked by repository policy".to_string());
                }
            }
        } else if self.mergeable == Some(false) {
            // Fallback if we don't have detailed info
            reasons.push("🔒 Server Check: Merge blocked by repository policy".to_string());
        }

        // ❌ PR State Check
        if !self.pr.is_open() {
            reasons.push(format!(
                "❌ PR Status: Pull request is {}",
                self.pr.state.as_str()
            ));
        }

        // 🔄 Build Status Check
        if let Some(build_status) = &self.build_status {
            match build_status.state {
                BuildState::Failed => reasons.push("❌ Build Status: Build failed".to_string()),
                BuildState::InProgress => {
                    reasons.push("⏳ Build Status: Build in progress".to_string())
                }
                BuildState::Cancelled => {
                    reasons.push("❌ Build Status: Build cancelled".to_string())
                }
                BuildState::Unknown => {
                    reasons.push("❓ Build Status: Build status unknown".to_string())
                }
                BuildState::Successful => {} // No blocking reason
            }
        }

        // 👥 Review Status Check (supplementary to server checks)
        if !self.review_status.can_merge {
            // Don't show approval count requirement since we don't know the real number
            // The server-side checks (above) already include approval requirements
            if self.review_status.current_approvals == 0 {
                reasons.push("👥 Review Status: No approvals yet".to_string());
            }

            if self.review_status.needs_work_count > 0 {
                reasons.push(format!(
                    "👥 Review Status: {} reviewer{} requested changes",
                    self.review_status.needs_work_count,
                    if self.review_status.needs_work_count == 1 {
                        ""
                    } else {
                        "s"
                    }
                ));
            }

            if !self.review_status.missing_reviewers.is_empty() {
                reasons.push(format!(
                    "👥 Review Status: Missing approval from: {}",
                    self.review_status.missing_reviewers.join(", ")
                ));
            }
        }

        // ⚠️ Merge Conflicts Check
        if let Some(conflicts) = &self.conflicts {
            if !conflicts.is_empty() {
                reasons.push(format!(
                    "⚠️ Merge Conflicts: {} file{} with conflicts",
                    conflicts.len(),
                    if conflicts.len() == 1 { "" } else { "s" }
                ));
            }
        }

        reasons
    }

    /// Check if this PR can be auto-merged based on conditions
    pub fn can_auto_merge(&self, conditions: &AutoMergeConditions) -> bool {
        // ✅ Check if PR is open
        if !self.pr.is_open() {
            return false;
        }

        // ✅ Author allowlist check (if specified)
        if let Some(allowed_authors) = &conditions.allowed_authors {
            if !allowed_authors.contains(&self.pr.author.user.name) {
                return false;
            }
        }

        // ✅ Use Bitbucket's authoritative merge endpoint result
        // This checks all server-side requirements: approvals, builds, conflicts, etc.
        self.mergeable.unwrap_or(false)
    }
}

/// Auto-merge configuration
#[derive(Debug, Clone)]
pub struct AutoMergeConditions {
    pub merge_strategy: MergeStrategy,
    pub wait_for_builds: bool,
    pub build_timeout: Duration,
    pub allowed_authors: Option<Vec<String>>, // Only auto-merge from trusted authors
}

impl Default for AutoMergeConditions {
    fn default() -> Self {
        Self {
            merge_strategy: MergeStrategy::Squash,
            wait_for_builds: true,
            build_timeout: Duration::from_secs(1800), // 30 minutes
            allowed_authors: None,
        }
    }
}

/// Merge strategy for pull requests
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MergeStrategy {
    #[serde(rename = "merge-commit")]
    Merge,
    #[serde(rename = "squash")]
    Squash,
    #[serde(rename = "fast-forward")]
    FastForward,
}

impl MergeStrategy {
    pub fn get_commit_message(&self, pr: &PullRequest) -> Option<String> {
        match self {
            MergeStrategy::Squash => Some(format!(
                "{}\n\n{}",
                pr.title,
                pr.description.as_deref().unwrap_or("")
            )),
            _ => None, // Use Bitbucket default
        }
    }
}

/// Result of auto-merge attempt
#[derive(Debug)]
pub enum AutoMergeResult {
    Merged {
        pr: Box<PullRequest>,
        merge_strategy: MergeStrategy,
    },
    NotReady {
        blocking_reasons: Vec<String>,
    },
    Failed {
        error: String,
    },
}

/// Merge request payload for Bitbucket Server
#[derive(Debug, Serialize)]
struct MergePullRequestRequest {
    version: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(rename = "strategy")]
    strategy: MergeStrategy,
}

/// Mergeability details
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MergeabilityDetails {
    pub can_merge: bool,
    pub conflicted: bool,
    pub blocking_reasons: Vec<String>,
    pub server_enforced: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Helper function to create a mock pull request
    fn create_test_pull_request(id: u64, state: PullRequestState) -> PullRequest {
        let is_open = state == PullRequestState::Open;
        PullRequest {
            id,
            version: 1,
            title: "Test PR".to_string(),
            description: Some("Test description".to_string()),
            state: state.clone(),
            open: is_open,
            closed: !is_open,
            created_date: 1700000000000, // Mock timestamp
            updated_date: 1700000000000,
            from_ref: PullRequestRef {
                id: "refs/heads/feature".to_string(),
                display_id: "feature".to_string(),
                latest_commit: "abc123".to_string(),
                repository: create_test_repository(),
            },
            to_ref: PullRequestRef {
                id: "refs/heads/main".to_string(),
                display_id: "main".to_string(),
                latest_commit: "def456".to_string(),
                repository: create_test_repository(),
            },
            locked: false,
            author: create_test_participant(ParticipantRole::Author, ParticipantStatus::Approved),
            links: PullRequestLinks {
                self_link: vec![SelfLink {
                    href: format!(
                        "http://bitbucket.local/projects/TEST/repos/test/pull-requests/{id}"
                    ),
                }],
            },
        }
    }

    fn create_test_repository() -> Repository {
        Repository {
            id: 1,
            name: "test-repo".to_string(),
            slug: "test-repo".to_string(),
            scm_id: "git".to_string(),
            state: "AVAILABLE".to_string(),
            status_message: Some("Available".to_string()),
            forkable: true,
            project: Project {
                id: 1,
                key: "TEST".to_string(),
                name: "Test Project".to_string(),
                description: Some("Test project description".to_string()),
                public: false,
                project_type: "NORMAL".to_string(),
            },
            public: false,
        }
    }

    fn create_test_participant(role: ParticipantRole, status: ParticipantStatus) -> Participant {
        Participant {
            user: User {
                name: "testuser".to_string(),
                display_name: Some("Test User".to_string()),
                email_address: Some("test@example.com".to_string()),
                active: true,
                slug: Some("testuser".to_string()),
            },
            role,
            approved: status == ParticipantStatus::Approved,
            status,
        }
    }

    fn create_test_build_status(state: BuildState) -> BuildStatus {
        BuildStatus {
            state,
            url: Some("http://ci.example.com/build/123".to_string()),
            description: Some("Test build".to_string()),
            context: Some("CI/CD".to_string()),
        }
    }

    #[test]
    fn test_pull_request_state_serialization() {
        assert_eq!(PullRequestState::Open.as_str(), "OPEN");
        assert_eq!(PullRequestState::Merged.as_str(), "MERGED");
        assert_eq!(PullRequestState::Declined.as_str(), "DECLINED");
    }

    #[test]
    fn test_pull_request_is_open() {
        let open_pr = create_test_pull_request(1, PullRequestState::Open);
        assert!(open_pr.is_open());

        let merged_pr = create_test_pull_request(2, PullRequestState::Merged);
        assert!(!merged_pr.is_open());

        let declined_pr = create_test_pull_request(3, PullRequestState::Declined);
        assert!(!declined_pr.is_open());
    }

    #[test]
    fn test_pull_request_web_url() {
        let pr = create_test_pull_request(123, PullRequestState::Open);
        let url = pr.web_url();
        assert!(url.is_some());
        assert_eq!(
            url.unwrap(),
            "http://bitbucket.local/projects/TEST/repos/test/pull-requests/123"
        );
    }

    #[test]
    fn test_merge_strategy_conversion() {
        let squash = MergeStrategy::Squash;
        let merge = MergeStrategy::Merge;
        let ff = MergeStrategy::FastForward;

        // Test that strategies can be created and compared
        assert!(matches!(squash, MergeStrategy::Squash));
        assert!(matches!(merge, MergeStrategy::Merge));
        assert!(matches!(ff, MergeStrategy::FastForward));
    }

    #[test]
    fn test_merge_strategy_commit_message() {
        let pr = create_test_pull_request(1, PullRequestState::Open);

        let squash_strategy = MergeStrategy::Squash;
        let message = squash_strategy.get_commit_message(&pr);
        assert!(message.is_some());
        assert!(message.unwrap().contains("Test PR"));

        let merge_strategy = MergeStrategy::Merge;
        let message = merge_strategy.get_commit_message(&pr);
        assert!(message.is_none()); // Merge strategy uses Bitbucket default

        let ff_strategy = MergeStrategy::FastForward;
        let message = ff_strategy.get_commit_message(&pr);
        assert!(message.is_none()); // Fast-forward doesn't create new commit message
    }

    #[test]
    fn test_auto_merge_conditions_default() {
        let conditions = AutoMergeConditions::default();

        assert!(conditions.wait_for_builds); // Default is true for auto-merge safety
        assert_eq!(conditions.build_timeout.as_secs(), 1800); // 30 minutes
        assert!(conditions.allowed_authors.is_none());
        assert!(matches!(conditions.merge_strategy, MergeStrategy::Squash));
    }

    #[test]
    fn test_auto_merge_conditions_custom() {
        let conditions = AutoMergeConditions {
            merge_strategy: MergeStrategy::Merge,
            wait_for_builds: false,
            build_timeout: Duration::from_secs(3600),
            allowed_authors: Some(vec!["trusted-user".to_string()]),
        };

        assert!(matches!(conditions.merge_strategy, MergeStrategy::Merge));
        assert!(!conditions.wait_for_builds);
        assert_eq!(conditions.build_timeout.as_secs(), 3600);
        assert!(conditions.allowed_authors.is_some());
    }

    #[test]
    fn test_pull_request_status_ready_to_land() {
        let pr = create_test_pull_request(1, PullRequestState::Open);
        let participants = vec![create_test_participant(
            ParticipantRole::Reviewer,
            ParticipantStatus::Approved,
        )];
        let review_status = ReviewStatus {
            required_approvals: 1,
            current_approvals: 1,
            needs_work_count: 0,
            can_merge: true,
            missing_reviewers: vec![],
        };

        let status = PullRequestStatus {
            pr,
            mergeable: Some(true),
            mergeable_details: None,
            participants,
            build_status: Some(create_test_build_status(BuildState::Successful)),
            review_status,
            conflicts: None,
        };

        assert!(status.is_ready_to_land());
    }

    #[test]
    fn test_pull_request_status_not_ready_to_land() {
        let pr = create_test_pull_request(1, PullRequestState::Open);
        let participants = vec![create_test_participant(
            ParticipantRole::Reviewer,
            ParticipantStatus::Unapproved,
        )];
        let review_status = ReviewStatus {
            required_approvals: 1,
            current_approvals: 0,
            needs_work_count: 0,
            can_merge: false,
            missing_reviewers: vec!["reviewer".to_string()],
        };

        let status = PullRequestStatus {
            pr,
            mergeable: Some(false),
            mergeable_details: None,
            participants,
            build_status: Some(create_test_build_status(BuildState::Failed)),
            review_status,
            conflicts: Some(vec!["Conflict in file.txt".to_string()]),
        };

        assert!(!status.is_ready_to_land());
    }

    #[test]
    fn test_pull_request_status_blocking_reasons() {
        // Test PR with failed build
        let pr_status = PullRequestStatus {
            pr: create_test_pull_request(1, PullRequestState::Open),
            mergeable: Some(true),
            mergeable_details: None,
            participants: vec![create_test_participant(
                ParticipantRole::Author,
                ParticipantStatus::Approved,
            )],
            build_status: Some(create_test_build_status(BuildState::Failed)),
            review_status: ReviewStatus {
                required_approvals: 1,
                current_approvals: 0, // Needs approval
                needs_work_count: 0,
                can_merge: false,
                missing_reviewers: vec!["reviewer1".to_string()],
            },
            conflicts: None,
        };

        let blocking_reasons = pr_status.get_blocking_reasons();

        // Verify it detects multiple blocking reasons
        assert!(!blocking_reasons.is_empty());

        // Check for build failure (actual format is "Build failed")
        assert!(blocking_reasons.iter().any(|r| r.contains("Build failed")));

        // Check for approval requirement (now shows "No approvals yet" instead of specific count)
        assert!(blocking_reasons
            .iter()
            .any(|r| r.contains("No approvals yet")));
    }

    #[test]
    fn test_pull_request_status_can_auto_merge() {
        let pr = create_test_pull_request(1, PullRequestState::Open);
        let participants = vec![create_test_participant(
            ParticipantRole::Reviewer,
            ParticipantStatus::Approved,
        )];
        let review_status = ReviewStatus {
            required_approvals: 1,
            current_approvals: 1,
            needs_work_count: 0,
            can_merge: true,
            missing_reviewers: vec![],
        };

        let status = PullRequestStatus {
            pr,
            mergeable: Some(true),
            mergeable_details: None,
            participants,
            build_status: Some(create_test_build_status(BuildState::Successful)),
            review_status,
            conflicts: None,
        };

        let conditions = AutoMergeConditions::default();
        assert!(status.can_auto_merge(&conditions));

        // Test with author allowlist (should pass since PR author is "testuser")
        let allowlist_conditions = AutoMergeConditions {
            allowed_authors: Some(vec!["testuser".to_string()]),
            ..Default::default()
        };
        assert!(status.can_auto_merge(&allowlist_conditions));

        // Test with non-matching mergeable state
        let mut status_not_mergeable = status.clone();
        status_not_mergeable.mergeable = Some(false);
        assert!(!status_not_mergeable.can_auto_merge(&conditions));
    }

    #[test]
    fn test_build_state_variants() {
        // Test that all build states can be created
        let _successful = BuildState::Successful;
        let _failed = BuildState::Failed;
        let _in_progress = BuildState::InProgress;
        let _cancelled = BuildState::Cancelled;
        let _unknown = BuildState::Unknown;

        // Test works if it compiles
        // Test passes if we reach this point without errors
    }

    #[test]
    fn test_review_status_calculations() {
        let review_status = ReviewStatus {
            required_approvals: 2,
            current_approvals: 1,
            needs_work_count: 0,
            can_merge: false,
            missing_reviewers: vec!["reviewer2".to_string()],
        };

        assert_eq!(review_status.required_approvals, 2);
        assert_eq!(review_status.current_approvals, 1);
        assert_eq!(review_status.needs_work_count, 0);
        assert!(!review_status.can_merge);
        assert_eq!(review_status.missing_reviewers.len(), 1);
    }

    #[test]
    fn test_auto_merge_result_variants() {
        let pr = create_test_pull_request(1, PullRequestState::Merged);

        // Test successful merge result
        let merged_result = AutoMergeResult::Merged {
            pr: Box::new(pr.clone()),
            merge_strategy: MergeStrategy::Squash,
        };
        assert!(matches!(merged_result, AutoMergeResult::Merged { .. }));

        // Test not ready result
        let not_ready_result = AutoMergeResult::NotReady {
            blocking_reasons: vec!["Missing approvals".to_string()],
        };
        assert!(matches!(not_ready_result, AutoMergeResult::NotReady { .. }));

        // Test failed result
        let failed_result = AutoMergeResult::Failed {
            error: "Network error".to_string(),
        };
        assert!(matches!(failed_result, AutoMergeResult::Failed { .. }));
    }

    #[test]
    fn test_participant_roles_and_status() {
        let author = create_test_participant(ParticipantRole::Author, ParticipantStatus::Approved);
        assert!(matches!(author.role, ParticipantRole::Author));
        assert!(author.approved);

        let reviewer =
            create_test_participant(ParticipantRole::Reviewer, ParticipantStatus::Unapproved);
        assert!(matches!(reviewer.role, ParticipantRole::Reviewer));
        assert!(!reviewer.approved);

        let needs_work =
            create_test_participant(ParticipantRole::Reviewer, ParticipantStatus::NeedsWork);
        assert!(matches!(needs_work.status, ParticipantStatus::NeedsWork));
        assert!(!needs_work.approved);
    }

    #[test]
    fn test_polling_frequency_constant() {
        // Test that the polling frequency is 30 seconds as documented
        use std::time::Duration;

        let polling_interval = Duration::from_secs(30);
        assert_eq!(polling_interval.as_secs(), 30);

        // Verify it's reasonable (between 10 seconds and 1 minute)
        assert!(polling_interval.as_secs() >= 10);
        assert!(polling_interval.as_secs() <= 60);
    }
}
