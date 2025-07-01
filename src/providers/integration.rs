use crate::config::CascadeConfig;
use crate::errors::{CascadeError, Result};
use crate::stack::{Stack, StackEntry, StackManager};
use crate::providers::{ProviderFactory, DynProvider, CreatePullRequestRequest, PullRequest, PullRequestStatus, BuildStatus};
use std::collections::HashMap;
use tracing::{info, warn};
use uuid::Uuid;

/// High-level integration between stacks and repository providers
pub struct ProviderIntegration {
    stack_manager: StackManager,
    provider: DynProvider,
    config: CascadeConfig,
}

/// Status of stack submissions across all entries
#[derive(Debug, Clone)]
pub struct StackSubmissionStatus {
    pub stack_name: String,
    pub total_entries: usize,
    pub submitted_entries: usize,
    pub open_prs: usize,
    pub merged_prs: usize,
    pub declined_prs: usize,
    pub pull_requests: Vec<PullRequest>,
    pub enhanced_statuses: Vec<EnhancedPullRequestStatus>,
    pub entry_statuses: HashMap<Uuid, EntrySubmissionStatus>,
}

/// Status of a single entry's submission
#[derive(Debug, Clone)]
pub struct EntrySubmissionStatus {
    pub entry_id: Uuid,
    pub branch_name: String,
    pub is_submitted: bool,
    pub pr_id: Option<String>,
    pub pr_status: Option<PullRequestStatus>,
    pub pr_url: Option<String>,
}

/// Enhanced pull request status with additional metadata
#[derive(Debug, Clone)]
pub struct EnhancedPullRequestStatus {
    pub pr: PullRequest,
    pub mergeable: Option<bool>,
    pub build_status: Option<BuildStatus>,
    pub is_ready_to_merge: bool,
}

impl EnhancedPullRequestStatus {
    /// Get display status string
    pub fn get_display_status(&self) -> String {
        let mut parts = Vec::new();
        
        // PR status
        match self.pr.status {
            PullRequestStatus::Open => parts.push("Open".to_string()),
            PullRequestStatus::Merged => parts.push("Merged".to_string()),
            PullRequestStatus::Declined => parts.push("Declined".to_string()),
            PullRequestStatus::Superseded => parts.push("Superseded".to_string()),
        }
        
        // Build status
        if let Some(build_status) = &self.build_status {
            match build_status {
                BuildStatus::Success => parts.push("✅ Build".to_string()),
                BuildStatus::Failed => parts.push("❌ Build".to_string()),
                BuildStatus::InProgress => parts.push("🔄 Build".to_string()),
                _ => parts.push("⚪ Build".to_string()),
            }
        }
        
        // Mergeable status
        if let Some(mergeable) = self.mergeable {
            if mergeable {
                parts.push("✅ Mergeable".to_string());
            } else {
                parts.push("❌ Conflicts".to_string());
            }
        }
        
        parts.join(" | ")
    }
    
    /// Check if PR is ready to land
    pub fn is_ready_to_land(&self) -> bool {
        self.is_ready_to_merge
    }
    
    /// Get blocking reasons if PR cannot be merged
    pub fn get_blocking_reasons(&self) -> Vec<String> {
        let mut reasons = Vec::new();
        
        // Check PR status
        match self.pr.status {
            PullRequestStatus::Merged => reasons.push("Already merged".to_string()),
            PullRequestStatus::Declined => reasons.push("PR was declined".to_string()),
            PullRequestStatus::Superseded => reasons.push("PR was superseded".to_string()),
            PullRequestStatus::Open => {} // Can potentially be merged
        }
        
        // Check build status
        if let Some(build_status) = &self.build_status {
            match build_status {
                BuildStatus::Failed => reasons.push("Build failed".to_string()),
                BuildStatus::InProgress => reasons.push("Build in progress".to_string()),
                BuildStatus::NotStarted => reasons.push("Build not started".to_string()),
                BuildStatus::Unknown => reasons.push("Build status unknown".to_string()),
                BuildStatus::Success => {} // No blocking reason
            }
        }
        
        // Check mergeable status
        if let Some(mergeable) = self.mergeable {
            if !mergeable {
                reasons.push("Has merge conflicts".to_string());
            }
        }
        
        reasons
    }
}

impl ProviderIntegration {
    /// Create a new provider integration using the configured provider
    pub fn new(stack_manager: StackManager, config: CascadeConfig) -> Result<Self> {
        let provider = ProviderFactory::create_provider(&config)?;
        
        Ok(Self {
            stack_manager,
            provider,
            config,
        })
    }

    /// Submit a single stack entry as a pull request
    pub async fn submit_entry(
        &mut self,
        stack_id: &Uuid,
        entry_id: &Uuid,
        title: Option<String>,
        description: Option<String>,
        draft: bool,
    ) -> Result<PullRequest> {
        let stack = self
            .stack_manager
            .get_stack(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?;

        let entry = stack
            .get_entry(entry_id)
            .ok_or_else(|| CascadeError::config(format!("Entry {entry_id} not found in stack")))?;

        info!("Submitting stack entry {} as pull request", entry_id);

        // Validate git integrity before submission
        if let Err(integrity_error) = stack.validate_git_integrity(self.stack_manager.git_repo()) {
            return Err(CascadeError::validation(format!(
                "Cannot submit entry from corrupted stack '{}':\n{}",
                stack.name, integrity_error
            )));
        }

        // Push branch to remote if not already pushed
        let git_repo = self.stack_manager.git_repo();
        info!("Ensuring branch '{}' is pushed to remote", entry.branch);

        match git_repo.push(&entry.branch) {
            Ok(_) => {
                info!("✅ Successfully pushed branch '{}' to remote", entry.branch);
            }
            Err(e) => {
                warn!("Failed to push branch '{}': {}", entry.branch, e);
                info!("Attempting to create PR anyway (branch may already exist remotely)");
            }
        }

        // Determine target branch (parent entry's branch or stack base)
        let target_branch = self.get_target_branch(stack, entry)?;

        // Ensure target branch is also pushed to remote (if it's not the base branch)
        if target_branch != stack.base_branch {
            info!("Ensuring target branch '{}' is pushed to remote", target_branch);
            match git_repo.push(&target_branch) {
                Ok(_) => {
                    info!("✅ Successfully pushed target branch '{}' to remote", target_branch);
                }
                Err(e) => {
                    warn!("Failed to push target branch '{}': {}", target_branch, e);
                    info!("Continuing anyway (target branch may already exist remotely)");
                }
            }
        }

        // Create pull request
        let pr_request = self.create_pr_request(stack, entry, &target_branch, title, description, draft)?;

        let pr = match self.provider.create_pull_request(pr_request).await {
            Ok(pr) => pr,
            Err(e) => {
                return Err(CascadeError::config(format!(
                    "Failed to create pull request for branch '{}' -> '{}': {}. \
                    Ensure both branches exist in the remote repository. \
                    You can manually push with: git push origin {}",
                    entry.branch, target_branch, e, entry.branch
                )));
            }
        };

        // Update stack manager with PR information
        self.stack_manager
            .submit_entry(stack_id, entry_id, pr.id.clone())?;

        info!("Created pull request #{} for entry {}", pr.id, entry_id);
        Ok(pr)
    }

    /// Check the status of all pull requests in a stack
    pub async fn check_stack_status(&self, stack_id: &Uuid) -> Result<StackSubmissionStatus> {
        let stack = self
            .stack_manager
            .get_stack(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?;

        let mut entry_statuses = HashMap::new();
        let mut submitted_count = 0;
        let mut open_count = 0;
        let mut merged_count = 0;
        let mut declined_count = 0;
        let mut pull_requests = Vec::new();
        let mut enhanced_statuses = Vec::new();

        for entry in &stack.entries {
            let mut entry_status = EntrySubmissionStatus {
                entry_id: entry.id,
                branch_name: entry.branch.clone(),
                is_submitted: entry.is_submitted,
                pr_id: entry.pull_request_id.clone(),
                pr_status: None,
                pr_url: None,
            };

            if let Some(pr_id) = &entry.pull_request_id {
                submitted_count += 1;
                
                match self.provider.get_pull_request(pr_id).await {
                    Ok(pr) => {
                        entry_status.pr_status = Some(pr.status.clone());
                        entry_status.pr_url = Some(pr.web_url.clone());
                        
                        match pr.status {
                            PullRequestStatus::Open => open_count += 1,
                            PullRequestStatus::Merged => merged_count += 1,
                            PullRequestStatus::Declined => declined_count += 1,
                            PullRequestStatus::Superseded => {} // Don't count superseded
                        }
                        
                        // Create enhanced status
                        let enhanced_status = EnhancedPullRequestStatus {
                            is_ready_to_merge: pr.status == PullRequestStatus::Open,
                            mergeable: Some(true), // TODO: Get actual mergeable status
                            build_status: None, // TODO: Get build status for commit
                            pr: pr.clone(),
                        };
                        enhanced_statuses.push(enhanced_status);
                        
                        pull_requests.push(pr);
                    }
                    Err(e) => {
                        warn!("Failed to get PR status for {}: {}", pr_id, e);
                    }
                }
            }

            entry_statuses.insert(entry.id, entry_status);
        }

        Ok(StackSubmissionStatus {
            stack_name: stack.name.clone(),
            total_entries: stack.entries.len(),
            submitted_entries: submitted_count,
            open_prs: open_count,
            merged_prs: merged_count,
            declined_prs: declined_count,
            pull_requests,
            enhanced_statuses,
            entry_statuses,
        })
    }

    /// List all pull requests for entries in the current stack
    pub async fn list_pull_requests(&self, stack_id: &Uuid) -> Result<Vec<PullRequest>> {
        let stack = self
            .stack_manager
            .get_stack(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?;

        let mut pull_requests = Vec::new();

        for entry in &stack.entries {
            if let Some(pr_id) = &entry.pull_request_id {
                match self.provider.get_pull_request(pr_id).await {
                    Ok(pr) => pull_requests.push(pr),
                    Err(e) => {
                        warn!("Failed to get pull request {}: {}", pr_id, e);
                    }
                }
            }
        }

        Ok(pull_requests)
    }

    /// Update PRs after rebase - check if branches still exist and update PR titles if needed
    pub async fn update_prs_after_rebase(&mut self, stack_id: &Uuid) -> Result<()> {
        let stack = self
            .stack_manager
            .get_stack(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?;

        for entry in &stack.entries {
            if let Some(pr_id) = &entry.pull_request_id {
                // Check if the branch still exists
                match self.provider.check_branch_exists(&entry.branch).await {
                    Ok(true) => {
                        info!("Branch '{}' still exists after rebase", entry.branch);
                    }
                    Ok(false) => {
                        warn!("Branch '{}' no longer exists after rebase, PR {} may need attention", 
                              entry.branch, pr_id);
                    }
                    Err(e) => {
                        warn!("Failed to check if branch '{}' exists: {}", entry.branch, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get enhanced status including build information (alias for backward compatibility)
    pub async fn check_enhanced_stack_status(&self, stack_id: &Uuid) -> Result<StackSubmissionStatus> {
        // For now, just return the basic status - enhanced features can be added later
        self.check_stack_status(stack_id).await
    }

    // Helper methods

    /// Get the target branch for a stack entry
    fn get_target_branch(&self, stack: &Stack, entry: &StackEntry) -> Result<String> {
        // If this is the first entry, target the base branch
        if let Some(parent_id) = &entry.parent_id {
            // Find the parent entry and use its branch
            if let Some(parent_entry) = stack.get_entry(parent_id) {
                Ok(parent_entry.branch.clone())
            } else {
                Err(CascadeError::config(format!(
                    "Parent entry {} not found for entry {}",
                    parent_id, entry.id
                )))
            }
        } else {
            // This is the first entry, target the base branch
            Ok(stack.base_branch.clone())
        }
    }

    /// Create a pull request request from stack entry
    fn create_pr_request(
        &self,
        stack: &Stack,
        entry: &StackEntry,
        target_branch: &str,
        title: Option<String>,
        description: Option<String>,
        _draft: bool, // TODO: Handle draft PRs in provider abstraction
    ) -> Result<CreatePullRequestRequest> {
        let pr_title = title.unwrap_or_else(|| {
            format!("[{}] {}", stack.name, entry.message)
        });

        let pr_description = description.unwrap_or_else(|| {
            format!(
                "Stack entry from stack: {}\n\nCommit: {}\nBranch: {}",
                stack.name, entry.commit_hash, entry.branch
            )
        });

        Ok(CreatePullRequestRequest {
            title: pr_title,
            description: pr_description,
            source_branch: entry.branch.clone(),
            target_branch: target_branch.to_string(),
            reviewers: self.config.bitbucket
                .as_ref()
                .map(|b| b.default_reviewers.clone())
                .unwrap_or_default(),
        })
    }
}

/// Enhanced stack status with build information
#[derive(Debug, Clone)]
pub struct EnhancedStackStatus {
    pub basic: StackSubmissionStatus,
    pub enhanced_entries: HashMap<Uuid, EnhancedEntryStatus>,
}

/// Enhanced entry status with build information
#[derive(Debug, Clone)]
pub struct EnhancedEntryStatus {
    pub basic: EntrySubmissionStatus,
    pub build_status: Option<crate::providers::BuildStatus>,
    pub is_ready_to_merge: bool,
}

impl StackSubmissionStatus {
    /// Calculate completion percentage
    pub fn completion_percentage(&self) -> f64 {
        if self.total_entries == 0 {
            0.0
        } else {
            (self.merged_prs as f64 / self.total_entries as f64) * 100.0
        }
    }

    /// Check if all entries are submitted
    pub fn all_submitted(&self) -> bool {
        self.submitted_entries == self.total_entries
    }

    /// Check if all PRs are merged
    pub fn all_merged(&self) -> bool {
        self.merged_prs == self.total_entries
    }
}