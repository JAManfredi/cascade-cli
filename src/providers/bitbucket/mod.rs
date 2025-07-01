use super::{
    BuildInfo, BuildStatus, CreatePullRequestRequest, MergeResult, MergeStrategy, ProviderHealth,
    ProviderType, PullRequest, PullRequestStatus, RepositoryInfo, RepositoryProvider,
    UpdatePullRequestRequest,
};
use crate::bitbucket::pull_request::{
    BuildState as BitbucketBuildState, MergeStrategy as BitbucketMergeStrategy,
};
use crate::bitbucket::{
    BitbucketClient, CreatePullRequestRequest as BitbucketCreateRequest,
    PullRequest as BitbucketPR, PullRequestManager, PullRequestState as BitbucketPRState,
};
use crate::config::BitbucketConfig;
use crate::errors::{CascadeError, Result};
use async_trait::async_trait;

/// Bitbucket Server provider implementation
pub struct BitbucketProvider {
    client: BitbucketClient,
    pr_manager: PullRequestManager,
    config: BitbucketConfig,
}

impl BitbucketProvider {
    /// Create a new Bitbucket provider
    pub fn new(config: BitbucketConfig) -> Result<Self> {
        let client = BitbucketClient::new(&config)?;
        let pr_manager = PullRequestManager::new(BitbucketClient::new(&config)?);
        Ok(Self {
            client,
            pr_manager,
            config,
        })
    }
}

#[async_trait]
impl RepositoryProvider for BitbucketProvider {
    fn name(&self) -> &'static str {
        "Bitbucket Server"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Bitbucket
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        // For now, perform a simple check by trying to access the API
        // TODO: Implement a proper health check endpoint
        match self.client.get::<serde_json::Value>("repos").await {
            Ok(_) => Ok(ProviderHealth {
                is_healthy: true,
                can_authenticate: true,
                can_create_prs: true,
                message: Some("Connected to Bitbucket Server".to_string()),
            }),
            Err(e) => Ok(ProviderHealth {
                is_healthy: false,
                can_authenticate: false,
                can_create_prs: false,
                message: Some(format!("Bitbucket connection failed: {}", e)),
            }),
        }
    }

    async fn get_repository_info(&self) -> Result<RepositoryInfo> {
        Ok(RepositoryInfo {
            provider: ProviderType::Bitbucket,
            url: self.config.url.clone(),
            project: self.config.project.clone(),
            repository: self.config.repo.clone(),
            default_branch: "main".to_string(), // TODO: Get from Bitbucket API
        })
    }

    async fn create_pull_request(&self, request: CreatePullRequestRequest) -> Result<PullRequest> {
        let bitbucket_request = convert_to_bitbucket_create_request(request)?;
        let bitbucket_pr = self
            .pr_manager
            .create_pull_request(bitbucket_request)
            .await?;
        convert_from_bitbucket_pr(bitbucket_pr)
    }

    async fn get_pull_request(&self, id: &str) -> Result<PullRequest> {
        let pr_id: u64 = id
            .parse()
            .map_err(|_| CascadeError::config(format!("Invalid PR ID: {}", id)))?;
        let bitbucket_pr = self.pr_manager.get_pull_request(pr_id).await?;
        convert_from_bitbucket_pr(bitbucket_pr)
    }

    async fn update_pull_request(
        &self,
        id: &str,
        update: UpdatePullRequestRequest,
    ) -> Result<PullRequest> {
        let _ = update; // Suppress unused variable warning
                        // Bitbucket doesn't support direct PR updates in the same way as other providers
                        // For now, just return the current PR - actual implementation would depend on what's being updated
                        // TODO: Implement title/description updates, reviewer changes, etc.
        self.get_pull_request(id).await
    }

    async fn decline_pull_request(&self, id: &str, reason: Option<String>) -> Result<()> {
        let pr_id: u64 = id
            .parse()
            .map_err(|_| CascadeError::config(format!("Invalid PR ID: {}", id)))?;
        let decline_reason =
            reason.unwrap_or_else(|| "Declined via provider abstraction".to_string());
        self.pr_manager
            .decline_pull_request(pr_id, &decline_reason)
            .await
    }

    async fn merge_pull_request(&self, id: &str, strategy: MergeStrategy) -> Result<MergeResult> {
        let pr_id: u64 = id
            .parse()
            .map_err(|_| CascadeError::config(format!("Invalid PR ID: {}", id)))?;
        let bitbucket_strategy = convert_to_bitbucket_merge_strategy(strategy.clone());
        let merged_pr = self
            .pr_manager
            .merge_pull_request(pr_id, bitbucket_strategy)
            .await?;

        Ok(MergeResult {
            merged_commit_hash: merged_pr.to_ref.latest_commit.clone(),
            message: Some(format!("Merged using {:?} strategy", strategy)),
        })
    }

    async fn check_branch_exists(&self, branch: &str) -> Result<bool> {
        // Use Bitbucket API to check if branch exists
        let path = format!("branches/{}", branch);
        match self.client.get::<serde_json::Value>(&path).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false), // Branch doesn't exist or we can't access it
        }
    }

    async fn get_build_status(&self, commit_hash: &str) -> Result<BuildInfo> {
        let path = format!("commits/{}/builds", commit_hash);

        #[derive(serde::Deserialize)]
        struct BuildResponse {
            values: Vec<BitbucketBuildInfo>,
        }

        #[derive(serde::Deserialize)]
        struct BitbucketBuildInfo {
            state: BitbucketBuildState,
            url: Option<String>,
            description: Option<String>,
            name: Option<String>,
        }

        match self.client.get::<BuildResponse>(&path).await {
            Ok(response) => {
                if let Some(build) = response.values.first() {
                    Ok(BuildInfo {
                        status: convert_bitbucket_build_status(build.state.clone()),
                        url: build.url.clone(),
                        description: build.description.clone(),
                        context: build.name.clone(),
                    })
                } else {
                    Ok(BuildInfo {
                        status: BuildStatus::Unknown,
                        url: None,
                        description: Some("No builds found".to_string()),
                        context: None,
                    })
                }
            }
            Err(_) => Ok(BuildInfo {
                status: BuildStatus::Unknown,
                url: None,
                description: Some("Build status unavailable".to_string()),
                context: None,
            }),
        }
    }

    async fn wait_for_builds(&self, commit_hash: &str, timeout_seconds: u64) -> Result<BuildInfo> {
        use tokio::time::{sleep, timeout, Duration};

        let timeout_duration = Duration::from_secs(timeout_seconds);

        timeout(timeout_duration, async {
            loop {
                let build_status = self.get_build_status(commit_hash).await?;

                match build_status.status {
                    BuildStatus::Success => return Ok(build_status),
                    BuildStatus::Failed => {
                        return Err(CascadeError::bitbucket(format!(
                            "Build failed: {}",
                            build_status.description.unwrap_or_default()
                        )));
                    }
                    BuildStatus::InProgress => {
                        sleep(Duration::from_secs(30)).await; // Poll every 30s
                        continue;
                    }
                    BuildStatus::Unknown => {
                        return Err(CascadeError::bitbucket("Build status unknown".to_string()));
                    }
                    _ => {
                        return Err(CascadeError::bitbucket(
                            "Unexpected build status".to_string(),
                        ));
                    }
                }
            }
        })
        .await
        .map_err(|_| CascadeError::bitbucket("Build timeout exceeded".to_string()))?
    }
}

// Conversion functions between provider abstraction types and Bitbucket types

/// Convert provider CreatePullRequestRequest to Bitbucket CreatePullRequestRequest
fn convert_to_bitbucket_create_request(
    request: CreatePullRequestRequest,
) -> Result<BitbucketCreateRequest> {
    Ok(BitbucketCreateRequest {
        title: request.title,
        description: Some(request.description),
        from_ref: crate::bitbucket::PullRequestRef {
            id: format!("refs/heads/{}", request.source_branch),
            display_id: request.source_branch,
            latest_commit: "HEAD".to_string(), // Bitbucket will resolve this
            repository: create_default_repository(), // TODO: Get from config
        },
        to_ref: crate::bitbucket::PullRequestRef {
            id: format!("refs/heads/{}", request.target_branch),
            display_id: request.target_branch,
            latest_commit: "HEAD".to_string(),
            repository: create_default_repository(),
        },
        draft: None,
    })
}

/// Convert Bitbucket PullRequest to provider PullRequest
fn convert_from_bitbucket_pr(pr: BitbucketPR) -> Result<PullRequest> {
    let web_url = pr.web_url().unwrap_or_default();
    let created_at = pr.created_at();
    let updated_at = pr.updated_at();

    Ok(PullRequest {
        id: pr.id.to_string(),
        title: pr.title,
        description: pr.description.unwrap_or_default(),
        source_branch: pr.from_ref.display_id,
        target_branch: pr.to_ref.display_id,
        author: pr.author.user.display_name,
        status: convert_bitbucket_pr_status(pr.state),
        web_url,
        reviewers: vec![], // TODO: Convert participants to reviewers
        created_at,
        updated_at,
    })
}

/// Convert Bitbucket PullRequestState to provider PullRequestStatus
fn convert_bitbucket_pr_status(state: BitbucketPRState) -> PullRequestStatus {
    match state {
        BitbucketPRState::Open => PullRequestStatus::Open,
        BitbucketPRState::Merged => PullRequestStatus::Merged,
        BitbucketPRState::Declined => PullRequestStatus::Declined,
    }
}

/// Convert Bitbucket BuildState to provider BuildStatus
fn convert_bitbucket_build_status(state: BitbucketBuildState) -> BuildStatus {
    match state {
        BitbucketBuildState::Successful => BuildStatus::Success,
        BitbucketBuildState::Failed => BuildStatus::Failed,
        BitbucketBuildState::InProgress => BuildStatus::InProgress,
        BitbucketBuildState::Cancelled => BuildStatus::Failed,
        BitbucketBuildState::Unknown => BuildStatus::Unknown,
    }
}

/// Convert provider MergeStrategy to Bitbucket MergeStrategy
fn convert_to_bitbucket_merge_strategy(strategy: MergeStrategy) -> BitbucketMergeStrategy {
    match strategy {
        MergeStrategy::Merge => BitbucketMergeStrategy::Merge,
        MergeStrategy::Squash => BitbucketMergeStrategy::Squash,
        MergeStrategy::FastForward => BitbucketMergeStrategy::FastForward,
        MergeStrategy::SquashFastForward => BitbucketMergeStrategy::Squash, // Map to closest equivalent
    }
}

/// Create a default repository for PR refs (temporary implementation)
fn create_default_repository() -> crate::bitbucket::Repository {
    crate::bitbucket::Repository {
        id: 1,
        name: "default".to_string(),
        slug: "default".to_string(),
        scm_id: "git".to_string(),
        state: "AVAILABLE".to_string(),
        status_message: "Available".to_string(),
        forkable: true,
        project: crate::bitbucket::Project {
            id: 1,
            key: "DEFAULT".to_string(),
            name: "Default Project".to_string(),
            description: None,
            public: false,
            project_type: "NORMAL".to_string(),
        },
        public: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitbucket::pull_request::BuildState as BitbucketBuildState;

    #[test]
    fn test_provider_type() {
        let config = BitbucketConfig {
            url: "https://bitbucket.example.com".to_string(),
            project: "TEST".to_string(),
            repo: "test-repo".to_string(),
            username: Some("test-user".to_string()),
            token: Some("test-token".to_string()),
            default_reviewers: vec![],
        };

        let provider = BitbucketProvider::new(config).unwrap();
        assert_eq!(provider.provider_type(), ProviderType::Bitbucket);
        assert_eq!(provider.name(), "Bitbucket Server");
    }

    #[test]
    fn test_build_status_conversion() {
        assert_eq!(
            convert_bitbucket_build_status(BitbucketBuildState::Successful),
            BuildStatus::Success
        );
        assert_eq!(
            convert_bitbucket_build_status(BitbucketBuildState::Failed),
            BuildStatus::Failed
        );
        assert_eq!(
            convert_bitbucket_build_status(BitbucketBuildState::InProgress),
            BuildStatus::InProgress
        );
    }
}
