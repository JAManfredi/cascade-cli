use crate::errors::{CascadeError, Result};
use crate::config::CascadeConfig;
use crate::stack::{Stack, StackEntry, StackManager};
use crate::bitbucket::client::BitbucketClient;
use crate::bitbucket::pull_request::{
    PullRequestManager, CreatePullRequestRequest, PullRequestRef, PullRequest, 
    Repository, Project, PullRequestState
};
use uuid::Uuid;
use tracing::{info, warn};

/// High-level integration between stacks and Bitbucket
pub struct BitbucketIntegration {
    stack_manager: StackManager,
    pr_manager: PullRequestManager,
    config: CascadeConfig,
}

impl BitbucketIntegration {
    /// Create a new Bitbucket integration
    pub fn new(stack_manager: StackManager, config: CascadeConfig) -> Result<Self> {
        let bitbucket_config = config.bitbucket.as_ref()
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
    pub async fn submit_entry(&mut self, stack_id: &Uuid, entry_id: &Uuid, title: Option<String>, description: Option<String>) -> Result<PullRequest> {
        let stack = self.stack_manager.get_stack(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {} not found", stack_id)))?;

        let entry = stack.get_entry(entry_id)
            .ok_or_else(|| CascadeError::config(format!("Entry {} not found in stack", entry_id)))?;

        info!("Submitting stack entry {} as pull request", entry_id);

        // Determine target branch (parent entry's branch or stack base)
        let target_branch = self.get_target_branch(stack, entry)?;

        // Create pull request
        let pr_request = self.create_pr_request(stack, entry, &target_branch, title, description)?;
        let pr = self.pr_manager.create_pull_request(pr_request).await?;

        // Update stack manager with PR information
        self.stack_manager.submit_entry(stack_id, entry_id, pr.id.to_string())?;

        info!("Created pull request #{} for entry {}", pr.id, entry_id);
        Ok(pr)
    }

    /// Check the status of all pull requests in a stack
    pub async fn check_stack_status(&self, stack_id: &Uuid) -> Result<StackSubmissionStatus> {
        let stack = self.stack_manager.get_stack(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {} not found", stack_id)))?;

        let mut status = StackSubmissionStatus {
            stack_name: stack.name.clone(),
            total_entries: stack.entries.len(),
            submitted_entries: 0,
            open_prs: 0,
            merged_prs: 0,
            declined_prs: 0,
            pull_requests: Vec::new(),
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
    pub async fn list_pull_requests(&self, state: Option<PullRequestState>) -> Result<crate::bitbucket::pull_request::PullRequestPage> {
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
        let entry_index = stack.entries.iter().position(|e| e.id == entry.id)
            .ok_or_else(|| CascadeError::config("Entry not found in stack"))?;

        if entry_index == 0 {
            Ok(stack.base_branch.clone())
        } else {
            Ok(stack.entries[entry_index - 1].branch.clone())
        }
    }

    /// Create a pull request request object
    fn create_pr_request(&self, _stack: &Stack, entry: &StackEntry, target_branch: &str, title: Option<String>, description: Option<String>) -> Result<CreatePullRequestRequest> {
        let bitbucket_config = self.config.bitbucket.as_ref().unwrap();

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
            id: format!("refs/heads/{}", target_branch),
            display_id: target_branch.to_string(),
            latest_commit: "".to_string(), // This will be filled by the API
            repository,
        };

        let title = title.unwrap_or_else(|| {
            entry.message.lines().next().unwrap_or("").to_string()
        });

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
        })
    }
}

/// Status of stack submission
#[derive(Debug)]
pub struct StackSubmissionStatus {
    pub stack_name: String,
    pub total_entries: usize,
    pub submitted_entries: usize,
    pub open_prs: usize,
    pub merged_prs: usize,
    pub declined_prs: usize,
    pub pull_requests: Vec<PullRequest>,
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
