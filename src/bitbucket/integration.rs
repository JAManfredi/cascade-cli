use crate::bitbucket::client::BitbucketClient;
use crate::bitbucket::pull_request::{
    CreatePullRequestRequest, Project, PullRequest, PullRequestManager, PullRequestRef,
    PullRequestState, Repository,
};
use crate::config::CascadeConfig;
use crate::errors::{CascadeError, Result};
use crate::stack::{Stack, StackEntry, StackManager};
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

/// High-level integration between stacks and Bitbucket
pub struct BitbucketIntegration {
    stack_manager: StackManager,
    pr_manager: PullRequestManager,
    config: CascadeConfig,
}

impl BitbucketIntegration {
    /// Create a new Bitbucket integration
    pub fn new(stack_manager: StackManager, config: CascadeConfig) -> Result<Self> {
        let bitbucket_config = config
            .bitbucket
            .as_ref()
            .ok_or_else(|| CascadeError::config("Bitbucket configuration not found"))?;

        let client = BitbucketClient::new(bitbucket_config)?;
        let pr_manager = PullRequestManager::new(client);

        Ok(Self {
            stack_manager,
            pr_manager,
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

        // Determine target branch (parent entry's branch or stack base)
        let target_branch = self.get_target_branch(stack, entry)?;

        // Create pull request
        let pr_request =
            self.create_pr_request(stack, entry, &target_branch, title, description, draft)?;
        let pr = self.pr_manager.create_pull_request(pr_request).await?;

        // Update stack manager with PR information
        self.stack_manager
            .submit_entry(stack_id, entry_id, pr.id.to_string())?;

        info!("Created pull request #{} for entry {}", pr.id, entry_id);
        Ok(pr)
    }

    /// Check the status of all pull requests in a stack
    pub async fn check_stack_status(&self, stack_id: &Uuid) -> Result<StackSubmissionStatus> {
        let stack = self
            .stack_manager
            .get_stack(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?;

        let mut status = StackSubmissionStatus {
            stack_name: stack.name.clone(),
            total_entries: stack.entries.len(),
            submitted_entries: 0,
            open_prs: 0,
            merged_prs: 0,
            declined_prs: 0,
            pull_requests: Vec::new(),
            enhanced_statuses: Vec::new(),
        };

        for entry in &stack.entries {
            if let Some(pr_id_str) = &entry.pull_request_id {
                status.submitted_entries += 1;

                if let Ok(pr_id) = pr_id_str.parse::<u64>() {
                    match self.pr_manager.get_pull_request(pr_id).await {
                        Ok(pr) => {
                            match pr.state {
                                PullRequestState::Open => status.open_prs += 1,
                                PullRequestState::Merged => status.merged_prs += 1,
                                PullRequestState::Declined => status.declined_prs += 1,
                            }
                            status.pull_requests.push(pr);
                        }
                        Err(e) => {
                            warn!("Failed to get pull request #{}: {}", pr_id, e);
                        }
                    }
                }
            }
        }

        Ok(status)
    }

    /// List all pull requests for the repository
    pub async fn list_pull_requests(
        &self,
        state: Option<PullRequestState>,
    ) -> Result<crate::bitbucket::pull_request::PullRequestPage> {
        self.pr_manager.list_pull_requests(state).await
    }

    /// Get the target branch for a stack entry
    fn get_target_branch(&self, stack: &Stack, entry: &StackEntry) -> Result<String> {
        // For the first entry (bottom of stack), target is the base branch
        if let Some(first_entry) = stack.entries.first() {
            if entry.id == first_entry.id {
                return Ok(stack.base_branch.clone());
            }
        }

        // For other entries, find the parent entry's branch
        let entry_index = stack
            .entries
            .iter()
            .position(|e| e.id == entry.id)
            .ok_or_else(|| CascadeError::config("Entry not found in stack"))?;

        if entry_index == 0 {
            Ok(stack.base_branch.clone())
        } else {
            Ok(stack.entries[entry_index - 1].branch.clone())
        }
    }

    /// Create a pull request request object
    fn create_pr_request(
        &self,
        _stack: &Stack,
        entry: &StackEntry,
        target_branch: &str,
        title: Option<String>,
        description: Option<String>,
        draft: bool,
    ) -> Result<CreatePullRequestRequest> {
        let bitbucket_config = self.config.bitbucket.as_ref()
            .ok_or_else(|| CascadeError::config("Bitbucket configuration is missing. Run 'cc setup' to configure Bitbucket integration."))?;

        let repository = Repository {
            id: 0, // This will be filled by the API
            name: bitbucket_config.repo.clone(),
            slug: bitbucket_config.repo.clone(),
            scm_id: "git".to_string(),
            state: "AVAILABLE".to_string(),
            status_message: "Available".to_string(),
            forkable: true,
            project: Project {
                id: 0,
                key: bitbucket_config.project.clone(),
                name: bitbucket_config.project.clone(),
                description: None,
                public: false,
                project_type: "NORMAL".to_string(),
            },
            public: false,
        };

        let from_ref = PullRequestRef {
            id: format!("refs/heads/{}", entry.branch),
            display_id: entry.branch.clone(),
            latest_commit: entry.commit_hash.clone(),
            repository: repository.clone(),
        };

        let to_ref = PullRequestRef {
            id: format!("refs/heads/{target_branch}"),
            display_id: target_branch.to_string(),
            latest_commit: "".to_string(), // This will be filled by the API
            repository,
        };

        let title = title.unwrap_or_else(|| entry.message.lines().next().unwrap_or("").to_string());

        let description = description.or_else(|| {
            if entry.message.lines().count() > 1 {
                Some(entry.message.lines().skip(1).collect::<Vec<_>>().join("\n"))
            } else {
                None
            }
        });

        Ok(CreatePullRequestRequest {
            title,
            description,
            from_ref,
            to_ref,
            draft: if draft { Some(true) } else { None },
        })
    }

    /// Update pull requests after a rebase using smart force push strategy
    /// This preserves all review history by updating existing branches instead of creating new ones
    pub async fn update_prs_after_rebase(
        &mut self,
        stack_id: &Uuid,
        branch_mapping: &HashMap<String, String>,
    ) -> Result<Vec<String>> {
        info!(
            "Updating pull requests after rebase for stack {} using smart force push",
            stack_id
        );

        let stack = self
            .stack_manager
            .get_stack(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?
            .clone();

        let mut updated_branches = Vec::new();

        for entry in &stack.entries {
            // Check if this entry has an existing PR and was remapped to a new branch
            if let (Some(pr_id_str), Some(new_branch)) =
                (&entry.pull_request_id, branch_mapping.get(&entry.branch))
            {
                if let Ok(pr_id) = pr_id_str.parse::<u64>() {
                    info!(
                        "Found existing PR #{} for entry {}, updating branch {} -> {}",
                        pr_id, entry.id, entry.branch, new_branch
                    );

                    // Get the existing PR to understand its current state
                    match self.pr_manager.get_pull_request(pr_id).await {
                        Ok(_existing_pr) => {
                            // Force push the new branch content to the old branch name
                            // This preserves the PR while updating its contents
                            match self
                                .stack_manager
                                .git_repo()
                                .force_push_branch(&entry.branch, new_branch)
                            {
                                Ok(_) => {
                                    info!(
                                        "✅ Successfully force-pushed {} to preserve PR #{}",
                                        entry.branch, pr_id
                                    );

                                    // Add a comment explaining the rebase
                                    let rebase_comment = format!(
                                        "🔄 **Automatic rebase completed**\n\n\
                                        This PR has been automatically rebased to incorporate the latest changes.\n\
                                        - Original commits: `{}`\n\
                                        - New base: Latest main branch\n\
                                        - All review history and comments are preserved\n\n\
                                        The changes in this PR remain the same - only the base has been updated.",
                                        &entry.commit_hash[..8]
                                    );

                                    if let Err(e) =
                                        self.pr_manager.add_comment(pr_id, &rebase_comment).await
                                    {
                                        warn!(
                                            "Failed to add rebase comment to PR #{}: {}",
                                            pr_id, e
                                        );
                                    }

                                    updated_branches.push(format!(
                                        "PR #{}: {} (preserved)",
                                        pr_id, entry.branch
                                    ));
                                }
                                Err(e) => {
                                    error!("Failed to force push {}: {}", entry.branch, e);
                                    // Fall back to creating a comment about the issue
                                    let error_comment = format!(
                                        "⚠️ **Rebase Update Issue**\n\n\
                                        The automatic rebase completed, but updating this PR failed.\n\
                                        You may need to manually update this branch.\n\
                                        Error: {e}"
                                    );

                                    if let Err(e2) =
                                        self.pr_manager.add_comment(pr_id, &error_comment).await
                                    {
                                        warn!(
                                            "Failed to add error comment to PR #{}: {}",
                                            pr_id, e2
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Could not retrieve PR #{}: {}", pr_id, e);
                        }
                    }
                }
            } else if branch_mapping.contains_key(&entry.branch) {
                // This entry was remapped but doesn't have a PR yet
                info!(
                    "Entry {} was remapped but has no PR - no action needed",
                    entry.id
                );
            }
        }

        if !updated_branches.is_empty() {
            info!(
                "Successfully updated {} PRs using smart force push strategy",
                updated_branches.len()
            );
        }

        Ok(updated_branches)
    }

    /// Check the enhanced status of all pull requests in a stack
    pub async fn check_enhanced_stack_status(
        &self,
        stack_id: &Uuid,
    ) -> Result<StackSubmissionStatus> {
        let stack = self
            .stack_manager
            .get_stack(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?;

        let mut status = StackSubmissionStatus {
            stack_name: stack.name.clone(),
            total_entries: stack.entries.len(),
            submitted_entries: 0,
            open_prs: 0,
            merged_prs: 0,
            declined_prs: 0,
            pull_requests: Vec::new(),
            enhanced_statuses: Vec::new(),
        };

        for entry in &stack.entries {
            if let Some(pr_id_str) = &entry.pull_request_id {
                status.submitted_entries += 1;

                if let Ok(pr_id) = pr_id_str.parse::<u64>() {
                    // Get enhanced status instead of basic PR
                    match self.pr_manager.get_pull_request_status(pr_id).await {
                        Ok(enhanced_status) => {
                            match enhanced_status.pr.state {
                                crate::bitbucket::pull_request::PullRequestState::Open => {
                                    status.open_prs += 1
                                }
                                crate::bitbucket::pull_request::PullRequestState::Merged => {
                                    status.merged_prs += 1
                                }
                                crate::bitbucket::pull_request::PullRequestState::Declined => {
                                    status.declined_prs += 1
                                }
                            }
                            status.pull_requests.push(enhanced_status.pr.clone());
                            status.enhanced_statuses.push(enhanced_status);
                        }
                        Err(e) => {
                            warn!("Failed to get enhanced status for PR #{}: {}", pr_id, e);
                            // Fallback to basic PR info
                            match self.pr_manager.get_pull_request(pr_id).await {
                                Ok(pr) => {
                                    match pr.state {
                                        crate::bitbucket::pull_request::PullRequestState::Open => status.open_prs += 1,
                                        crate::bitbucket::pull_request::PullRequestState::Merged => status.merged_prs += 1,
                                        crate::bitbucket::pull_request::PullRequestState::Declined => status.declined_prs += 1,
                                    }
                                    status.pull_requests.push(pr);
                                }
                                Err(e2) => {
                                    warn!("Failed to get basic PR #{}: {}", pr_id, e2);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(status)
    }
}

/// Status of stack submission with enhanced mergability information
#[derive(Debug)]
pub struct StackSubmissionStatus {
    pub stack_name: String,
    pub total_entries: usize,
    pub submitted_entries: usize,
    pub open_prs: usize,
    pub merged_prs: usize,
    pub declined_prs: usize,
    pub pull_requests: Vec<PullRequest>,
    pub enhanced_statuses: Vec<crate::bitbucket::pull_request::PullRequestStatus>,
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
