pub mod types;
pub mod factory;
pub mod bitbucket;
pub mod integration;

use async_trait::async_trait;
use crate::errors::Result;
pub use types::*;
pub use factory::ProviderFactory;
pub use integration::{ProviderIntegration, StackSubmissionStatus, EntrySubmissionStatus, EnhancedStackStatus, EnhancedEntryStatus, EnhancedPullRequestStatus};

/// Main trait defining repository provider operations
#[async_trait]
pub trait RepositoryProvider: Send + Sync {
    /// Get the provider name for display purposes
    fn name(&self) -> &'static str;
    
    /// Get the provider type
    fn provider_type(&self) -> ProviderType;
    
    /// Check if the provider is healthy and can perform operations
    async fn health_check(&self) -> Result<ProviderHealth>;
    
    /// Get repository information
    async fn get_repository_info(&self) -> Result<RepositoryInfo>;
    
    /// Pull Request Operations
    
    /// Create a new pull request
    async fn create_pull_request(&self, request: CreatePullRequestRequest) -> Result<PullRequest>;
    
    /// Get an existing pull request by ID
    async fn get_pull_request(&self, id: &str) -> Result<PullRequest>;
    
    /// Update an existing pull request
    async fn update_pull_request(&self, id: &str, update: UpdatePullRequestRequest) -> Result<PullRequest>;
    
    /// Decline/close a pull request
    async fn decline_pull_request(&self, id: &str, reason: Option<String>) -> Result<()>;
    
    /// Merge a pull request
    async fn merge_pull_request(&self, id: &str, strategy: MergeStrategy) -> Result<MergeResult>;
    
    /// Branch Operations
    
    /// Check if a branch exists in the remote repository
    async fn check_branch_exists(&self, branch: &str) -> Result<bool>;
    
    /// Build Status Operations
    
    /// Get the current build status for a commit
    async fn get_build_status(&self, commit_hash: &str) -> Result<BuildInfo>;
    
    /// Wait for builds to complete on a commit (with timeout)
    async fn wait_for_builds(&self, commit_hash: &str, timeout_seconds: u64) -> Result<BuildInfo>;
}

/// Type alias for boxed provider
pub type DynProvider = Box<dyn RepositoryProvider>;

/// Trait for provider configuration
pub trait ProviderConfig {
    fn provider_type(&self) -> ProviderType;
    fn validate(&self) -> Result<()>;
}