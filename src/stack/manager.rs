use super::metadata::RepositoryMetadata;
use super::{CommitMetadata, Stack, StackEntry, StackMetadata, StackStatus};
use crate::config::get_repo_config_dir;
use crate::errors::{CascadeError, Result};
use crate::git::GitRepository;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use uuid::Uuid;

/// Types of branch modifications detected during Git integrity checks
#[derive(Debug)]
pub enum BranchModification {
    /// Branch is missing (needs to be created)
    Missing {
        branch: String,
        entry_id: Uuid,
        expected_commit: String,
    },
    /// Branch has extra commits beyond what's expected
    ExtraCommits {
        branch: String,
        entry_id: Uuid,
        expected_commit: String,
        actual_commit: String,
        extra_commit_count: usize,
        extra_commit_messages: Vec<String>,
    },
}

/// Manages all stack operations and persistence
pub struct StackManager {
    /// Git repository interface
    repo: GitRepository,
    /// Path to the repository root
    repo_path: PathBuf,
    /// Path to cascade config directory
    config_dir: PathBuf,
    /// Path to stacks data file
    stacks_file: PathBuf,
    /// Path to metadata file
    metadata_file: PathBuf,
    /// In-memory stack data
    stacks: HashMap<Uuid, Stack>,
    /// Repository metadata
    metadata: RepositoryMetadata,
}

impl StackManager {
    /// Create a new StackManager for the given repository
    pub fn new(repo_path: &Path) -> Result<Self> {
        let repo = GitRepository::open(repo_path)?;
        let config_dir = get_repo_config_dir(repo_path)?;
        let stacks_file = config_dir.join("stacks.json");
        let metadata_file = config_dir.join("metadata.json");

        // Determine default base branch - try current branch first, then check for common defaults
        let default_base = match repo.get_current_branch() {
            Ok(branch) => branch,
            Err(_) => {
                // Fallback: check if common default branches exist
                if repo.branch_exists("main") {
                    "main".to_string()
                } else if repo.branch_exists("master") {
                    "master".to_string()
                } else {
                    // Final fallback to main (modern Git default)
                    "main".to_string()
                }
            }
        };

        let mut manager = Self {
            repo,
            repo_path: repo_path.to_path_buf(),
            config_dir,
            stacks_file,
            metadata_file,
            stacks: HashMap::new(),
            metadata: RepositoryMetadata::new(default_base),
        };

        // Load existing data if available
        manager.load_from_disk()?;

        Ok(manager)
    }

    /// Create a new stack
    pub fn create_stack(
        &mut self,
        name: String,
        base_branch: Option<String>,
        description: Option<String>,
    ) -> Result<Uuid> {
        // Check if stack with this name already exists
        if self.metadata.find_stack_by_name(&name).is_some() {
            return Err(CascadeError::config(format!(
                "Stack '{name}' already exists"
            )));
        }

        // Use provided base branch or default
        let base_branch = base_branch.unwrap_or_else(|| self.metadata.default_base_branch.clone());

        // Verify base branch exists (try to fetch from remote if not local)
        if !self.repo.branch_exists_or_fetch(&base_branch)? {
            return Err(CascadeError::branch(format!(
                "Base branch '{base_branch}' does not exist locally or remotely"
            )));
        }

        // Get current branch as the working branch
        let current_branch = self.repo.get_current_branch().ok();

        // Create the stack
        let mut stack = Stack::new(name.clone(), base_branch.clone(), description.clone());

        // Set working branch if we're on a feature branch (not on base branch)
        if let Some(ref branch) = current_branch {
            if branch != &base_branch {
                stack.working_branch = Some(branch.clone());
            }
        }

        let stack_id = stack.id;

        // Create metadata
        let stack_metadata = StackMetadata::new(stack_id, name, base_branch, description);

        // Store in memory
        self.stacks.insert(stack_id, stack);
        self.metadata.add_stack(stack_metadata);

        // Always set newly created stack as active
        self.set_active_stack(Some(stack_id))?;

        Ok(stack_id)
    }

    /// Get a stack by ID
    pub fn get_stack(&self, stack_id: &Uuid) -> Option<&Stack> {
        self.stacks.get(stack_id)
    }

    /// Get a mutable stack by ID
    pub fn get_stack_mut(&mut self, stack_id: &Uuid) -> Option<&mut Stack> {
        self.stacks.get_mut(stack_id)
    }

    /// Get stack by name
    pub fn get_stack_by_name(&self, name: &str) -> Option<&Stack> {
        if let Some(metadata) = self.metadata.find_stack_by_name(name) {
            self.stacks.get(&metadata.stack_id)
        } else {
            None
        }
    }

    /// Get mutable stack by name
    pub fn get_stack_by_name_mut(&mut self, name: &str) -> Option<&mut Stack> {
        if let Some(metadata) = self.metadata.find_stack_by_name(name) {
            self.stacks.get_mut(&metadata.stack_id)
        } else {
            None
        }
    }

    /// Update working branch for a stack
    pub fn update_stack_working_branch(&mut self, name: &str, branch: String) -> Result<()> {
        if let Some(stack) = self.get_stack_by_name_mut(name) {
            stack.working_branch = Some(branch);
            self.save_to_disk()?;
            Ok(())
        } else {
            Err(CascadeError::config(format!("Stack '{name}' not found")))
        }
    }

    /// Get the currently active stack
    pub fn get_active_stack(&self) -> Option<&Stack> {
        self.metadata
            .active_stack_id
            .and_then(|id| self.stacks.get(&id))
    }

    /// Get the currently active stack mutably
    pub fn get_active_stack_mut(&mut self) -> Option<&mut Stack> {
        if let Some(id) = self.metadata.active_stack_id {
            self.stacks.get_mut(&id)
        } else {
            None
        }
    }

    /// Set the active stack
    pub fn set_active_stack(&mut self, stack_id: Option<Uuid>) -> Result<()> {
        // Verify stack exists if provided
        if let Some(id) = stack_id {
            if !self.stacks.contains_key(&id) {
                return Err(CascadeError::config(format!(
                    "Stack with ID {id} not found"
                )));
            }
        }

        // Update active flag on stacks
        for stack in self.stacks.values_mut() {
            stack.set_active(Some(stack.id) == stack_id);
        }

        // Track the current branch when activating a stack
        if let Some(id) = stack_id {
            let current_branch = self.repo.get_current_branch().ok();
            if let Some(stack_meta) = self.metadata.get_stack_mut(&id) {
                stack_meta.set_current_branch(current_branch);
            }
        }

        self.metadata.set_active_stack(stack_id);
        self.save_to_disk()?;

        Ok(())
    }

    /// Set active stack by name
    pub fn set_active_stack_by_name(&mut self, name: &str) -> Result<()> {
        if let Some(metadata) = self.metadata.find_stack_by_name(name) {
            self.set_active_stack(Some(metadata.stack_id))
        } else {
            Err(CascadeError::config(format!("Stack '{name}' not found")))
        }
    }

    /// Delete a stack
    pub fn delete_stack(&mut self, stack_id: &Uuid) -> Result<Stack> {
        let stack = self
            .stacks
            .remove(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack with ID {stack_id} not found")))?;

        // Remove metadata
        self.metadata.remove_stack(stack_id);

        // Remove all associated commit metadata
        let stack_commits: Vec<String> = self
            .metadata
            .commits
            .values()
            .filter(|commit| &commit.stack_id == stack_id)
            .map(|commit| commit.hash.clone())
            .collect();

        for commit_hash in stack_commits {
            self.metadata.remove_commit(&commit_hash);
        }

        // If this was the active stack, find a new one
        if self.metadata.active_stack_id == Some(*stack_id) {
            let new_active = self.metadata.stacks.keys().next().copied();
            self.set_active_stack(new_active)?;
        }

        self.save_to_disk()?;

        Ok(stack)
    }

    /// Push a commit to a stack
    pub fn push_to_stack(
        &mut self,
        branch: String,
        commit_hash: String,
        message: String,
        source_branch: String,
    ) -> Result<Uuid> {
        let stack_id = self
            .metadata
            .active_stack_id
            .ok_or_else(|| CascadeError::config("No active stack"))?;

        let stack = self
            .stacks
            .get_mut(&stack_id)
            .ok_or_else(|| CascadeError::config("Active stack not found"))?;

        // 🆕 VALIDATE GIT INTEGRITY BEFORE PUSHING (if stack is not empty)
        if !stack.entries.is_empty() {
            if let Err(integrity_error) = stack.validate_git_integrity(&self.repo) {
                return Err(CascadeError::validation(format!(
                    "Cannot push to corrupted stack '{}':\n{}\n\n\
                     💡 Fix the stack integrity issues first using 'ca stack validate {}' for details.",
                    stack.name, integrity_error, stack.name
                )));
            }
        }

        // Verify the commit exists
        if !self.repo.commit_exists(&commit_hash)? {
            return Err(CascadeError::branch(format!(
                "Commit {commit_hash} does not exist"
            )));
        }

        // Check for duplicate commit messages within the same stack
        if let Some(duplicate_entry) = stack.entries.iter().find(|entry| entry.message == message) {
            return Err(CascadeError::validation(format!(
                "Duplicate commit message in stack: \"{message}\"\n\n\
                 This message already exists in entry {} (commit: {})\n\n\
                 💡 Consider using a more specific message:\n\
                    • Add context: \"{message} - add validation\"\n\
                    • Be more specific: \"Fix user authentication timeout bug\"\n\
                    • Or amend the previous commit: git commit --amend",
                duplicate_entry.id,
                &duplicate_entry.commit_hash[..8]
            )));
        }

        // 🎯 SMART BASE BRANCH UPDATE FOR FEATURE WORKFLOW
        // If this is the first commit in an empty stack, and the user is on a feature branch
        // that's different from the stack's base branch, update the base branch to match
        // the current workflow.
        if stack.entries.is_empty() {
            let current_branch = self.repo.get_current_branch()?;

            // Update working branch if not already set
            if stack.working_branch.is_none() && current_branch != stack.base_branch {
                stack.working_branch = Some(current_branch.clone());
                tracing::info!(
                    "Set working branch for stack '{}' to '{}'",
                    stack.name,
                    current_branch
                );
            }

            if current_branch != stack.base_branch && current_branch != "HEAD" {
                // Check if current branch was created from the stack's base branch
                let base_exists = self.repo.branch_exists(&stack.base_branch);
                let current_is_feature = current_branch.starts_with("feature/")
                    || current_branch.starts_with("fix/")
                    || current_branch.starts_with("chore/")
                    || current_branch.contains("feature")
                    || current_branch.contains("fix");

                if base_exists && current_is_feature {
                    tracing::info!(
                        "🎯 First commit detected: updating stack '{}' base branch from '{}' to '{}'",
                        stack.name, stack.base_branch, current_branch
                    );

                    println!("🎯 Smart Base Branch Update:");
                    println!(
                        "   Stack '{}' was created with base '{}'",
                        stack.name, stack.base_branch
                    );
                    println!("   You're now working on feature branch '{current_branch}'");
                    println!("   Updating stack base branch to match your workflow");

                    // Update the stack's base branch
                    stack.base_branch = current_branch.clone();

                    // Update metadata as well
                    if let Some(stack_meta) = self.metadata.get_stack_mut(&stack_id) {
                        stack_meta.base_branch = current_branch.clone();
                        stack_meta.set_current_branch(Some(current_branch.clone()));
                    }

                    println!(
                        "   ✅ Stack '{}' base branch updated to '{current_branch}'",
                        stack.name
                    );
                }
            }
        }

        // 🆕 CREATE ACTUAL GIT BRANCH from the specific commit
        // Check if branch already exists
        if self.repo.branch_exists(&branch) {
            tracing::info!("Branch '{}' already exists, skipping creation", branch);
        } else {
            // Create the branch from the specific commit hash
            self.repo
                .create_branch(&branch, Some(&commit_hash))
                .map_err(|e| {
                    CascadeError::branch(format!(
                        "Failed to create branch '{}' from commit {}: {}",
                        branch,
                        &commit_hash[..8],
                        e
                    ))
                })?;

            tracing::info!(
                "✅ Created Git branch '{}' from commit {}",
                branch,
                &commit_hash[..8]
            );
        }

        // Add to stack
        let entry_id = stack.push_entry(branch.clone(), commit_hash.clone(), message.clone());

        // Create commit metadata
        let commit_metadata = CommitMetadata::new(
            commit_hash.clone(),
            message,
            entry_id,
            stack_id,
            branch.clone(),
            source_branch,
        );

        // Update repository metadata
        self.metadata.add_commit(commit_metadata);
        if let Some(stack_meta) = self.metadata.get_stack_mut(&stack_id) {
            stack_meta.add_branch(branch);
            stack_meta.add_commit(commit_hash);
        }

        self.save_to_disk()?;

        Ok(entry_id)
    }

    /// Pop the top commit from the active stack
    pub fn pop_from_stack(&mut self) -> Result<StackEntry> {
        let stack_id = self
            .metadata
            .active_stack_id
            .ok_or_else(|| CascadeError::config("No active stack"))?;

        let stack = self
            .stacks
            .get_mut(&stack_id)
            .ok_or_else(|| CascadeError::config("Active stack not found"))?;

        let entry = stack
            .pop_entry()
            .ok_or_else(|| CascadeError::config("Stack is empty"))?;

        // Remove commit metadata
        self.metadata.remove_commit(&entry.commit_hash);

        // Update stack metadata
        if let Some(stack_meta) = self.metadata.get_stack_mut(&stack_id) {
            stack_meta.remove_commit(&entry.commit_hash);
            // Note: We don't remove the branch as there might be other commits on it
        }

        self.save_to_disk()?;

        Ok(entry)
    }

    /// Submit a stack entry for review (mark as submitted)
    pub fn submit_entry(
        &mut self,
        stack_id: &Uuid,
        entry_id: &Uuid,
        pull_request_id: String,
    ) -> Result<()> {
        let stack = self
            .stacks
            .get_mut(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?;

        let entry_commit_hash = {
            let entry = stack
                .get_entry(entry_id)
                .ok_or_else(|| CascadeError::config(format!("Entry {entry_id} not found")))?;
            entry.commit_hash.clone()
        };

        // Update stack entry
        if !stack.mark_entry_submitted(entry_id, pull_request_id.clone()) {
            return Err(CascadeError::config(format!(
                "Failed to mark entry {entry_id} as submitted"
            )));
        }

        // Update commit metadata
        if let Some(commit_meta) = self.metadata.commits.get_mut(&entry_commit_hash) {
            commit_meta.mark_submitted(pull_request_id);
        }

        // Update stack metadata statistics
        if let Some(stack_meta) = self.metadata.get_stack_mut(stack_id) {
            let submitted_count = stack.entries.iter().filter(|e| e.is_submitted).count();
            stack_meta.update_stats(
                stack.entries.len(),
                submitted_count,
                stack_meta.merged_commits,
            );
        }

        self.save_to_disk()?;

        Ok(())
    }

    /// Repair data consistency issues in all stacks
    pub fn repair_all_stacks(&mut self) -> Result<()> {
        for stack in self.stacks.values_mut() {
            stack.repair_data_consistency();
        }
        self.save_to_disk()?;
        Ok(())
    }

    /// Get all stacks
    pub fn get_all_stacks(&self) -> Vec<&Stack> {
        self.stacks.values().collect()
    }

    /// Get stack metadata
    pub fn get_stack_metadata(&self, stack_id: &Uuid) -> Option<&StackMetadata> {
        self.metadata.get_stack(stack_id)
    }

    /// Get repository metadata
    pub fn get_repository_metadata(&self) -> &RepositoryMetadata {
        &self.metadata
    }

    /// Get the Git repository
    pub fn git_repo(&self) -> &GitRepository {
        &self.repo
    }

    /// Get the repository path
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    // Edit mode management methods

    /// Check if currently in edit mode
    pub fn is_in_edit_mode(&self) -> bool {
        self.metadata
            .edit_mode
            .as_ref()
            .map(|edit_state| edit_state.is_active)
            .unwrap_or(false)
    }

    /// Get current edit mode information
    pub fn get_edit_mode_info(&self) -> Option<&super::metadata::EditModeState> {
        self.metadata.edit_mode.as_ref()
    }

    /// Enter edit mode for a specific stack entry
    pub fn enter_edit_mode(&mut self, stack_id: Uuid, entry_id: Uuid) -> Result<()> {
        // Get the commit hash first to avoid borrow checker issues
        let commit_hash = {
            let stack = self
                .get_stack(&stack_id)
                .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?;

            let entry = stack.get_entry(&entry_id).ok_or_else(|| {
                CascadeError::config(format!("Entry {entry_id} not found in stack"))
            })?;

            entry.commit_hash.clone()
        };

        // If already in edit mode, exit the current one first
        if self.is_in_edit_mode() {
            self.exit_edit_mode()?;
        }

        // Create new edit mode state
        let edit_state = super::metadata::EditModeState::new(stack_id, entry_id, commit_hash);

        self.metadata.edit_mode = Some(edit_state);
        self.save_to_disk()?;

        info!(
            "Entered edit mode for entry {} in stack {}",
            entry_id, stack_id
        );
        Ok(())
    }

    /// Exit edit mode
    pub fn exit_edit_mode(&mut self) -> Result<()> {
        if !self.is_in_edit_mode() {
            return Err(CascadeError::config("Not currently in edit mode"));
        }

        // Clear edit mode state
        self.metadata.edit_mode = None;
        self.save_to_disk()?;

        info!("Exited edit mode");
        Ok(())
    }

    /// Sync stack with Git repository state
    pub fn sync_stack(&mut self, stack_id: &Uuid) -> Result<()> {
        let stack = self
            .stacks
            .get_mut(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?;

        // 🆕 ENHANCED: Check Git integrity first (branch HEAD matches stored commits)
        if let Err(integrity_error) = stack.validate_git_integrity(&self.repo) {
            stack.update_status(StackStatus::Corrupted);
            return Err(CascadeError::branch(format!(
                "Stack '{}' Git integrity check failed:\n{}",
                stack.name, integrity_error
            )));
        }

        // Check if all commits still exist
        let mut missing_commits = Vec::new();
        for entry in &stack.entries {
            if !self.repo.commit_exists(&entry.commit_hash)? {
                missing_commits.push(entry.commit_hash.clone());
            }
        }

        if !missing_commits.is_empty() {
            stack.update_status(StackStatus::Corrupted);
            return Err(CascadeError::branch(format!(
                "Stack {} has missing commits: {}",
                stack.name,
                missing_commits.join(", ")
            )));
        }

        // Check if base branch exists and has new commits (try to fetch from remote if not local)
        if !self.repo.branch_exists_or_fetch(&stack.base_branch)? {
            return Err(CascadeError::branch(format!(
                "Base branch '{}' does not exist locally or remotely. Check the branch name or switch to a different base.",
                stack.base_branch
            )));
        }

        let _base_hash = self.repo.get_branch_head(&stack.base_branch)?;

        // Check if any stack entries are missing their commits
        let mut corrupted_entry = None;
        for entry in &stack.entries {
            if !self.repo.commit_exists(&entry.commit_hash)? {
                corrupted_entry = Some((entry.commit_hash.clone(), entry.branch.clone()));
                break;
            }
        }

        if let Some((commit_hash, branch)) = corrupted_entry {
            stack.update_status(StackStatus::Corrupted);
            return Err(CascadeError::branch(format!(
                "Commit {commit_hash} from stack entry '{branch}' no longer exists"
            )));
        }

        // Compare base branch with the earliest commit in the stack
        let needs_sync = if let Some(first_entry) = stack.entries.first() {
            // Get commits between base and first entry
            match self
                .repo
                .get_commits_between(&stack.base_branch, &first_entry.commit_hash)
            {
                Ok(commits) => !commits.is_empty(), // If there are commits, we need to sync
                Err(_) => true,                     // If we can't compare, assume we need to sync
            }
        } else {
            false // Empty stack is always clean
        };

        // Update stack status based on sync needs
        if needs_sync {
            stack.update_status(StackStatus::NeedsSync);
            info!(
                "Stack '{}' needs sync - new commits on base branch",
                stack.name
            );
        } else {
            stack.update_status(StackStatus::Clean);
            info!("Stack '{}' is clean", stack.name);
        }

        // Update metadata
        if let Some(stack_meta) = self.metadata.get_stack_mut(stack_id) {
            stack_meta.set_up_to_date(true);
        }

        self.save_to_disk()?;

        Ok(())
    }

    /// List all stacks with their status
    pub fn list_stacks(&self) -> Vec<(Uuid, &str, &StackStatus, usize, Option<&str>)> {
        self.stacks
            .values()
            .map(|stack| {
                (
                    stack.id,
                    stack.name.as_str(),
                    &stack.status,
                    stack.entries.len(),
                    if stack.is_active {
                        Some("active")
                    } else {
                        None
                    },
                )
            })
            .collect()
    }

    /// Get all stacks as Stack objects for TUI
    pub fn get_all_stacks_objects(&self) -> Result<Vec<Stack>> {
        let mut stacks: Vec<Stack> = self.stacks.values().cloned().collect();
        stacks.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(stacks)
    }

    /// Validate all stacks including Git integrity
    pub fn validate_all(&self) -> Result<()> {
        for stack in self.stacks.values() {
            // Basic structure validation
            stack.validate().map_err(|e| {
                CascadeError::config(format!("Stack '{}' validation failed: {}", stack.name, e))
            })?;

            // Git integrity validation
            stack.validate_git_integrity(&self.repo).map_err(|e| {
                CascadeError::config(format!(
                    "Stack '{}' Git integrity validation failed: {}",
                    stack.name, e
                ))
            })?;
        }
        Ok(())
    }

    /// Validate a specific stack including Git integrity
    pub fn validate_stack(&self, stack_id: &Uuid) -> Result<()> {
        let stack = self
            .stacks
            .get(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?;

        // Basic structure validation
        stack.validate().map_err(|e| {
            CascadeError::config(format!("Stack '{}' validation failed: {}", stack.name, e))
        })?;

        // Git integrity validation
        stack.validate_git_integrity(&self.repo).map_err(|e| {
            CascadeError::config(format!(
                "Stack '{}' Git integrity validation failed: {}",
                stack.name, e
            ))
        })?;

        Ok(())
    }

    /// Save all data to disk
    fn save_to_disk(&self) -> Result<()> {
        // Ensure config directory exists
        if !self.config_dir.exists() {
            fs::create_dir_all(&self.config_dir).map_err(|e| {
                CascadeError::config(format!("Failed to create config directory: {e}"))
            })?;
        }

        // Save stacks atomically
        crate::utils::atomic_file::write_json(&self.stacks_file, &self.stacks)?;

        // Save metadata atomically
        crate::utils::atomic_file::write_json(&self.metadata_file, &self.metadata)?;

        Ok(())
    }

    /// Load data from disk
    fn load_from_disk(&mut self) -> Result<()> {
        // Load stacks if file exists
        if self.stacks_file.exists() {
            let stacks_content = fs::read_to_string(&self.stacks_file)
                .map_err(|e| CascadeError::config(format!("Failed to read stacks file: {e}")))?;

            self.stacks = serde_json::from_str(&stacks_content)
                .map_err(|e| CascadeError::config(format!("Failed to parse stacks file: {e}")))?;
        }

        // Load metadata if file exists
        if self.metadata_file.exists() {
            let metadata_content = fs::read_to_string(&self.metadata_file)
                .map_err(|e| CascadeError::config(format!("Failed to read metadata file: {e}")))?;

            self.metadata = serde_json::from_str(&metadata_content)
                .map_err(|e| CascadeError::config(format!("Failed to parse metadata file: {e}")))?;
        }

        Ok(())
    }

    /// Check if the user has changed branches since the stack was activated
    /// Returns true if branch change detected and user wants to proceed
    pub fn check_for_branch_change(&mut self) -> Result<bool> {
        // Extract stack information first to avoid borrow conflicts
        let (stack_id, stack_name, stored_branch) = {
            if let Some(active_stack) = self.get_active_stack() {
                let stack_id = active_stack.id;
                let stack_name = active_stack.name.clone();
                let stored_branch = if let Some(stack_meta) = self.metadata.get_stack(&stack_id) {
                    stack_meta.current_branch.clone()
                } else {
                    None
                };
                (Some(stack_id), stack_name, stored_branch)
            } else {
                (None, String::new(), None)
            }
        };

        // If no active stack, nothing to check
        let Some(stack_id) = stack_id else {
            return Ok(true);
        };

        let current_branch = self.repo.get_current_branch().ok();

        // Check if branch has changed
        if stored_branch.as_ref() != current_branch.as_ref() {
            println!("⚠️  Branch change detected!");
            println!(
                "   Stack '{}' was active on: {}",
                stack_name,
                stored_branch.as_deref().unwrap_or("unknown")
            );
            println!(
                "   Current branch: {}",
                current_branch.as_deref().unwrap_or("unknown")
            );
            println!();
            println!("What would you like to do?");
            println!("   1. Keep stack '{stack_name}' active (continue with stack workflow)");
            println!("   2. Deactivate stack (use normal Git workflow)");
            println!("   3. Switch to a different stack");
            println!("   4. Cancel and stay on current workflow");
            print!("   Choice (1-4): ");

            use std::io::{self, Write};
            io::stdout()
                .flush()
                .map_err(|e| CascadeError::config(format!("Failed to write to stdout: {e}")))?;

            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(|e| CascadeError::config(format!("Failed to read user input: {e}")))?;

            match input.trim() {
                "1" => {
                    // Update the tracked branch and continue
                    if let Some(stack_meta) = self.metadata.get_stack_mut(&stack_id) {
                        stack_meta.set_current_branch(current_branch);
                    }
                    self.save_to_disk()?;
                    println!("✅ Continuing with stack '{stack_name}' on current branch");
                    return Ok(true);
                }
                "2" => {
                    // Deactivate the stack
                    self.set_active_stack(None)?;
                    println!("✅ Deactivated stack '{stack_name}' - using normal Git workflow");
                    return Ok(false);
                }
                "3" => {
                    // Show available stacks
                    let stacks = self.get_all_stacks();
                    if stacks.len() <= 1 {
                        println!("⚠️  No other stacks available. Deactivating current stack.");
                        self.set_active_stack(None)?;
                        return Ok(false);
                    }

                    println!("\nAvailable stacks:");
                    for (i, stack) in stacks.iter().enumerate() {
                        if stack.id != stack_id {
                            println!("   {}. {}", i + 1, stack.name);
                        }
                    }
                    print!("   Enter stack name: ");
                    io::stdout().flush().map_err(|e| {
                        CascadeError::config(format!("Failed to write to stdout: {e}"))
                    })?;

                    let mut stack_name_input = String::new();
                    io::stdin().read_line(&mut stack_name_input).map_err(|e| {
                        CascadeError::config(format!("Failed to read user input: {e}"))
                    })?;
                    let stack_name_input = stack_name_input.trim();

                    if let Err(e) = self.set_active_stack_by_name(stack_name_input) {
                        println!("⚠️  {e}");
                        println!("   Deactivating stack instead.");
                        self.set_active_stack(None)?;
                        return Ok(false);
                    } else {
                        println!("✅ Switched to stack '{stack_name_input}'");
                        return Ok(true);
                    }
                }
                _ => {
                    println!("Cancelled - no changes made");
                    return Ok(false);
                }
            }
        }

        // No branch change detected
        Ok(true)
    }

    /// Handle Git integrity issues with multiple user-friendly options
    /// Provides non-destructive choices for dealing with branch modifications
    pub fn handle_branch_modifications(
        &mut self,
        stack_id: &Uuid,
        auto_mode: Option<String>,
    ) -> Result<()> {
        let stack = self
            .stacks
            .get_mut(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?;

        info!("Checking Git integrity for stack '{}'", stack.name);

        // Detect all modifications
        let mut modifications = Vec::new();
        for entry in &stack.entries {
            if !self.repo.branch_exists(&entry.branch) {
                modifications.push(BranchModification::Missing {
                    branch: entry.branch.clone(),
                    entry_id: entry.id,
                    expected_commit: entry.commit_hash.clone(),
                });
            } else if let Ok(branch_head) = self.repo.get_branch_head(&entry.branch) {
                if branch_head != entry.commit_hash {
                    // Get extra commits and their messages
                    let extra_commits = self
                        .repo
                        .get_commits_between(&entry.commit_hash, &branch_head)?;
                    let mut extra_messages = Vec::new();
                    for commit in &extra_commits {
                        if let Some(message) = commit.message() {
                            let first_line =
                                message.lines().next().unwrap_or("(no message)").to_string();
                            extra_messages.push(format!(
                                "{}: {}",
                                &commit.id().to_string()[..8],
                                first_line
                            ));
                        }
                    }

                    modifications.push(BranchModification::ExtraCommits {
                        branch: entry.branch.clone(),
                        entry_id: entry.id,
                        expected_commit: entry.commit_hash.clone(),
                        actual_commit: branch_head,
                        extra_commit_count: extra_commits.len(),
                        extra_commit_messages: extra_messages,
                    });
                }
            }
        }

        if modifications.is_empty() {
            println!("✅ Stack '{}' has no Git integrity issues", stack.name);
            return Ok(());
        }

        // Show detected modifications
        println!(
            "🔍 Detected branch modifications in stack '{}':",
            stack.name
        );
        for (i, modification) in modifications.iter().enumerate() {
            match modification {
                BranchModification::Missing { branch, .. } => {
                    println!("   {}. Branch '{}' is missing", i + 1, branch);
                }
                BranchModification::ExtraCommits {
                    branch,
                    expected_commit,
                    actual_commit,
                    extra_commit_count,
                    extra_commit_messages,
                    ..
                } => {
                    println!(
                        "   {}. Branch '{}' has {} extra commit(s)",
                        i + 1,
                        branch,
                        extra_commit_count
                    );
                    println!(
                        "      Expected: {} | Actual: {}",
                        &expected_commit[..8],
                        &actual_commit[..8]
                    );

                    // Show extra commit messages (first few only)
                    for (j, message) in extra_commit_messages.iter().enumerate() {
                        if j < 3 {
                            println!("         + {message}");
                        } else if j == 3 {
                            println!("         + ... and {} more", extra_commit_count - 3);
                            break;
                        }
                    }
                }
            }
        }
        println!();

        // Auto mode handling
        if let Some(mode) = auto_mode {
            return self.apply_auto_fix(stack_id, &modifications, &mode);
        }

        // Interactive mode - ask user for each modification
        for modification in modifications {
            self.handle_single_modification(stack_id, &modification)?;
        }

        self.save_to_disk()?;
        println!("🎉 All branch modifications handled successfully!");
        Ok(())
    }

    /// Handle a single branch modification interactively
    fn handle_single_modification(
        &mut self,
        stack_id: &Uuid,
        modification: &BranchModification,
    ) -> Result<()> {
        match modification {
            BranchModification::Missing {
                branch,
                expected_commit,
                ..
            } => {
                println!("🔧 Missing branch '{branch}'");
                println!(
                    "   This will create the branch at commit {}",
                    &expected_commit[..8]
                );

                self.repo.create_branch(branch, Some(expected_commit))?;
                println!("   ✅ Created branch '{branch}'");
            }

            BranchModification::ExtraCommits {
                branch,
                entry_id,
                expected_commit,
                extra_commit_count,
                ..
            } => {
                println!(
                    "🤔 Branch '{branch}' has {extra_commit_count} extra commit(s). What would you like to do?"
                );
                println!("   1. 📝 Incorporate - Update stack entry to include extra commits");
                println!("   2. ➕ Split - Create new stack entry for extra commits");
                println!("   3. 🗑️  Reset - Remove extra commits (DESTRUCTIVE)");
                println!("   4. ⏭️  Skip - Leave as-is for now");
                print!("   Choice (1-4): ");

                use std::io::{self, Write};
                io::stdout().flush().unwrap();

                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();

                match input.trim() {
                    "1" | "incorporate" | "inc" => {
                        self.incorporate_extra_commits(stack_id, *entry_id, branch)?;
                    }
                    "2" | "split" | "new" => {
                        self.split_extra_commits(stack_id, *entry_id, branch)?;
                    }
                    "3" | "reset" | "remove" => {
                        self.reset_branch_destructive(branch, expected_commit)?;
                    }
                    "4" | "skip" | "ignore" => {
                        println!("   ⏭️  Skipping '{branch}' (integrity issue remains)");
                    }
                    _ => {
                        println!("   ❌ Invalid choice. Skipping '{branch}'");
                    }
                }
            }
        }

        Ok(())
    }

    /// Apply automatic fix based on mode
    fn apply_auto_fix(
        &mut self,
        stack_id: &Uuid,
        modifications: &[BranchModification],
        mode: &str,
    ) -> Result<()> {
        println!("🤖 Applying automatic fix mode: {mode}");

        for modification in modifications {
            match (modification, mode) {
                (
                    BranchModification::Missing {
                        branch,
                        expected_commit,
                        ..
                    },
                    _,
                ) => {
                    self.repo.create_branch(branch, Some(expected_commit))?;
                    println!("   ✅ Created missing branch '{branch}'");
                }

                (
                    BranchModification::ExtraCommits {
                        branch, entry_id, ..
                    },
                    "incorporate",
                ) => {
                    self.incorporate_extra_commits(stack_id, *entry_id, branch)?;
                }

                (
                    BranchModification::ExtraCommits {
                        branch, entry_id, ..
                    },
                    "split",
                ) => {
                    self.split_extra_commits(stack_id, *entry_id, branch)?;
                }

                (
                    BranchModification::ExtraCommits {
                        branch,
                        expected_commit,
                        ..
                    },
                    "reset",
                ) => {
                    self.reset_branch_destructive(branch, expected_commit)?;
                }

                _ => {
                    return Err(CascadeError::config(format!(
                        "Unknown auto-fix mode '{mode}'. Use: incorporate, split, reset"
                    )));
                }
            }
        }

        self.save_to_disk()?;
        println!("🎉 Auto-fix completed for mode: {mode}");
        Ok(())
    }

    /// Incorporate extra commits into the existing stack entry (update commit hash)
    fn incorporate_extra_commits(
        &mut self,
        stack_id: &Uuid,
        entry_id: Uuid,
        branch: &str,
    ) -> Result<()> {
        let stack = self.stacks.get_mut(stack_id).unwrap();

        if let Some(entry) = stack.entries.iter_mut().find(|e| e.id == entry_id) {
            let new_head = self.repo.get_branch_head(branch)?;
            let old_commit = entry.commit_hash[..8].to_string(); // Clone to avoid borrowing issue

            // Get the extra commits for message update
            let extra_commits = self
                .repo
                .get_commits_between(&entry.commit_hash, &new_head)?;

            // Update the entry to point to the new HEAD
            entry.commit_hash = new_head.clone();

            // Update commit message to reflect the incorporation
            let mut extra_messages = Vec::new();
            for commit in &extra_commits {
                if let Some(message) = commit.message() {
                    let first_line = message.lines().next().unwrap_or("").to_string();
                    extra_messages.push(first_line);
                }
            }

            if !extra_messages.is_empty() {
                entry.message = format!(
                    "{}\n\nIncorporated commits:\n• {}",
                    entry.message,
                    extra_messages.join("\n• ")
                );
            }

            println!(
                "   ✅ Incorporated {} commit(s) into entry '{}'",
                extra_commits.len(),
                entry.short_hash()
            );
            println!("      Updated: {} -> {}", old_commit, &new_head[..8]);
        }

        Ok(())
    }

    /// Split extra commits into a new stack entry
    fn split_extra_commits(&mut self, stack_id: &Uuid, entry_id: Uuid, branch: &str) -> Result<()> {
        let stack = self.stacks.get_mut(stack_id).unwrap();
        let new_head = self.repo.get_branch_head(branch)?;

        // Find the position of the current entry
        let entry_position = stack
            .entries
            .iter()
            .position(|e| e.id == entry_id)
            .ok_or_else(|| CascadeError::config("Entry not found in stack"))?;

        // Create a new branch name for the split
        let base_name = branch.trim_end_matches(|c: char| c.is_ascii_digit() || c == '-');
        let new_branch = format!("{base_name}-continued");

        // Create new branch at the current HEAD
        self.repo.create_branch(&new_branch, Some(&new_head))?;

        // Get extra commits for message creation
        let original_entry = &stack.entries[entry_position];
        let original_commit_hash = original_entry.commit_hash.clone(); // Clone to avoid borrowing issue
        let extra_commits = self
            .repo
            .get_commits_between(&original_commit_hash, &new_head)?;

        // Create commit message from extra commits
        let mut extra_messages = Vec::new();
        for commit in &extra_commits {
            if let Some(message) = commit.message() {
                let first_line = message.lines().next().unwrap_or("").to_string();
                extra_messages.push(first_line);
            }
        }

        let new_message = if extra_messages.len() == 1 {
            extra_messages[0].clone()
        } else {
            format!("Combined changes:\n• {}", extra_messages.join("\n• "))
        };

        // Create new stack entry manually (no constructor method exists)
        let now = chrono::Utc::now();
        let new_entry = crate::stack::StackEntry {
            id: uuid::Uuid::new_v4(),
            branch: new_branch.clone(),
            commit_hash: new_head,
            message: new_message,
            parent_id: Some(entry_id), // Parent is the current entry
            children: Vec::new(),
            created_at: now,
            updated_at: now,
            is_submitted: false,
            pull_request_id: None,
            is_synced: false,
        };

        // Insert the new entry after the current one
        stack.entries.insert(entry_position + 1, new_entry);

        // Reset the original branch to its expected commit
        self.repo
            .reset_branch_to_commit(branch, &original_commit_hash)?;

        println!(
            "   ✅ Split {} commit(s) into new entry '{}'",
            extra_commits.len(),
            new_branch
        );
        println!("      Original branch '{branch}' reset to expected commit");

        Ok(())
    }

    /// Reset branch to expected commit (destructive - loses extra work)
    fn reset_branch_destructive(&self, branch: &str, expected_commit: &str) -> Result<()> {
        self.repo.reset_branch_to_commit(branch, expected_commit)?;
        println!(
            "   ⚠️  Reset branch '{}' to {} (extra commits lost)",
            branch,
            &expected_commit[..8]
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repository
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Configure git
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Create an initial commit
        std::fs::write(repo_path.join("README.md"), "# Test Repo").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Initialize cascade
        crate::config::initialize_repo(&repo_path, Some("https://test.bitbucket.com".to_string()))
            .unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_create_stack_manager() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = StackManager::new(&repo_path).unwrap();

        assert_eq!(manager.stacks.len(), 0);
        assert!(manager.get_active_stack().is_none());
    }

    #[test]
    fn test_create_and_manage_stack() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut manager = StackManager::new(&repo_path).unwrap();

        // Create a stack using the default branch
        let stack_id = manager
            .create_stack(
                "test-stack".to_string(),
                None, // Use default branch
                Some("Test stack description".to_string()),
            )
            .unwrap();

        // Verify stack was created
        assert_eq!(manager.stacks.len(), 1);
        let stack = manager.get_stack(&stack_id).unwrap();
        assert_eq!(stack.name, "test-stack");
        // Should use the default branch (which gets set from the Git repo)
        assert!(!stack.base_branch.is_empty());
        assert!(stack.is_active);

        // Verify it's the active stack
        let active = manager.get_active_stack().unwrap();
        assert_eq!(active.id, stack_id);

        // Test get by name
        let found = manager.get_stack_by_name("test-stack").unwrap();
        assert_eq!(found.id, stack_id);
    }

    #[test]
    fn test_stack_persistence() {
        let (_temp_dir, repo_path) = create_test_repo();

        let stack_id = {
            let mut manager = StackManager::new(&repo_path).unwrap();
            manager
                .create_stack("persistent-stack".to_string(), None, None)
                .unwrap()
        };

        // Create new manager and verify data was loaded
        let manager = StackManager::new(&repo_path).unwrap();
        assert_eq!(manager.stacks.len(), 1);
        let stack = manager.get_stack(&stack_id).unwrap();
        assert_eq!(stack.name, "persistent-stack");
    }

    #[test]
    fn test_multiple_stacks() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut manager = StackManager::new(&repo_path).unwrap();

        let stack1_id = manager
            .create_stack("stack-1".to_string(), None, None)
            .unwrap();
        let stack2_id = manager
            .create_stack("stack-2".to_string(), None, None)
            .unwrap();

        assert_eq!(manager.stacks.len(), 2);

        // Second stack should be active (newly created stacks become active)
        assert!(!manager.get_stack(&stack1_id).unwrap().is_active);
        assert!(manager.get_stack(&stack2_id).unwrap().is_active);

        // Change active stack
        manager.set_active_stack(Some(stack2_id)).unwrap();
        assert!(!manager.get_stack(&stack1_id).unwrap().is_active);
        assert!(manager.get_stack(&stack2_id).unwrap().is_active);
    }

    #[test]
    fn test_delete_stack() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut manager = StackManager::new(&repo_path).unwrap();

        let stack_id = manager
            .create_stack("to-delete".to_string(), None, None)
            .unwrap();
        assert_eq!(manager.stacks.len(), 1);

        let deleted = manager.delete_stack(&stack_id).unwrap();
        assert_eq!(deleted.name, "to-delete");
        assert_eq!(manager.stacks.len(), 0);
        assert!(manager.get_active_stack().is_none());
    }

    #[test]
    fn test_validation() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut manager = StackManager::new(&repo_path).unwrap();

        manager
            .create_stack("valid-stack".to_string(), None, None)
            .unwrap();

        // Should pass validation
        assert!(manager.validate_all().is_ok());
    }

    #[test]
    fn test_duplicate_commit_message_detection() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut manager = StackManager::new(&repo_path).unwrap();

        // Create a stack
        manager
            .create_stack("test-stack".to_string(), None, None)
            .unwrap();

        // Create first commit
        std::fs::write(repo_path.join("file1.txt"), "content1").unwrap();
        Command::new("git")
            .args(["add", "file1.txt"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Add authentication feature"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let commit1_hash = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let commit1_hash = String::from_utf8_lossy(&commit1_hash.stdout)
            .trim()
            .to_string();

        // Push first commit to stack - should succeed
        let entry1_id = manager
            .push_to_stack(
                "feature/auth".to_string(),
                commit1_hash,
                "Add authentication feature".to_string(),
                "main".to_string(),
            )
            .unwrap();

        // Verify first entry was added
        assert!(manager
            .get_active_stack()
            .unwrap()
            .get_entry(&entry1_id)
            .is_some());

        // Create second commit
        std::fs::write(repo_path.join("file2.txt"), "content2").unwrap();
        Command::new("git")
            .args(["add", "file2.txt"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Different commit message"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let commit2_hash = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let commit2_hash = String::from_utf8_lossy(&commit2_hash.stdout)
            .trim()
            .to_string();

        // Try to push second commit with the SAME message - should fail
        let result = manager.push_to_stack(
            "feature/auth2".to_string(),
            commit2_hash.clone(),
            "Add authentication feature".to_string(), // Same message as first commit
            "main".to_string(),
        );

        // Should fail with validation error
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, CascadeError::Validation(_)));

        // Error message should contain helpful information
        let error_msg = error.to_string();
        assert!(error_msg.contains("Duplicate commit message"));
        assert!(error_msg.contains("Add authentication feature"));
        assert!(error_msg.contains("💡 Consider using a more specific message"));

        // Push with different message - should succeed
        let entry2_id = manager
            .push_to_stack(
                "feature/auth2".to_string(),
                commit2_hash,
                "Add authentication validation".to_string(), // Different message
                "main".to_string(),
            )
            .unwrap();

        // Verify both entries exist
        let stack = manager.get_active_stack().unwrap();
        assert_eq!(stack.entries.len(), 2);
        assert!(stack.get_entry(&entry1_id).is_some());
        assert!(stack.get_entry(&entry2_id).is_some());
    }

    #[test]
    fn test_duplicate_message_with_different_case() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut manager = StackManager::new(&repo_path).unwrap();

        manager
            .create_stack("test-stack".to_string(), None, None)
            .unwrap();

        // Create and push first commit
        std::fs::write(repo_path.join("file1.txt"), "content1").unwrap();
        Command::new("git")
            .args(["add", "file1.txt"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "fix bug"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let commit1_hash = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let commit1_hash = String::from_utf8_lossy(&commit1_hash.stdout)
            .trim()
            .to_string();

        manager
            .push_to_stack(
                "feature/fix1".to_string(),
                commit1_hash,
                "fix bug".to_string(),
                "main".to_string(),
            )
            .unwrap();

        // Create second commit
        std::fs::write(repo_path.join("file2.txt"), "content2").unwrap();
        Command::new("git")
            .args(["add", "file2.txt"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Fix Bug"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let commit2_hash = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let commit2_hash = String::from_utf8_lossy(&commit2_hash.stdout)
            .trim()
            .to_string();

        // Different case should be allowed (case-sensitive comparison)
        let result = manager.push_to_stack(
            "feature/fix2".to_string(),
            commit2_hash,
            "Fix Bug".to_string(), // Different case
            "main".to_string(),
        );

        // Should succeed because it's case-sensitive
        assert!(result.is_ok());
    }

    #[test]
    fn test_duplicate_message_across_different_stacks() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut manager = StackManager::new(&repo_path).unwrap();

        // Create first stack and push commit
        let stack1_id = manager
            .create_stack("stack1".to_string(), None, None)
            .unwrap();

        std::fs::write(repo_path.join("file1.txt"), "content1").unwrap();
        Command::new("git")
            .args(["add", "file1.txt"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "shared message"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let commit1_hash = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let commit1_hash = String::from_utf8_lossy(&commit1_hash.stdout)
            .trim()
            .to_string();

        manager
            .push_to_stack(
                "feature/shared1".to_string(),
                commit1_hash,
                "shared message".to_string(),
                "main".to_string(),
            )
            .unwrap();

        // Create second stack
        let stack2_id = manager
            .create_stack("stack2".to_string(), None, None)
            .unwrap();

        // Set second stack as active
        manager.set_active_stack(Some(stack2_id)).unwrap();

        // Create commit for second stack
        std::fs::write(repo_path.join("file2.txt"), "content2").unwrap();
        Command::new("git")
            .args(["add", "file2.txt"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "shared message"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let commit2_hash = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let commit2_hash = String::from_utf8_lossy(&commit2_hash.stdout)
            .trim()
            .to_string();

        // Same message in different stack should be allowed
        let result = manager.push_to_stack(
            "feature/shared2".to_string(),
            commit2_hash,
            "shared message".to_string(), // Same message but different stack
            "main".to_string(),
        );

        // Should succeed because it's a different stack
        assert!(result.is_ok());

        // Verify both stacks have entries with the same message
        let stack1 = manager.get_stack(&stack1_id).unwrap();
        let stack2 = manager.get_stack(&stack2_id).unwrap();

        assert_eq!(stack1.entries.len(), 1);
        assert_eq!(stack2.entries.len(), 1);
        assert_eq!(stack1.entries[0].message, "shared message");
        assert_eq!(stack2.entries[0].message, "shared message");
    }

    #[test]
    fn test_duplicate_after_pop() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut manager = StackManager::new(&repo_path).unwrap();

        manager
            .create_stack("test-stack".to_string(), None, None)
            .unwrap();

        // Create and push first commit
        std::fs::write(repo_path.join("file1.txt"), "content1").unwrap();
        Command::new("git")
            .args(["add", "file1.txt"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "temporary message"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let commit1_hash = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let commit1_hash = String::from_utf8_lossy(&commit1_hash.stdout)
            .trim()
            .to_string();

        manager
            .push_to_stack(
                "feature/temp".to_string(),
                commit1_hash,
                "temporary message".to_string(),
                "main".to_string(),
            )
            .unwrap();

        // Pop the entry
        let popped = manager.pop_from_stack().unwrap();
        assert_eq!(popped.message, "temporary message");

        // Create new commit
        std::fs::write(repo_path.join("file2.txt"), "content2").unwrap();
        Command::new("git")
            .args(["add", "file2.txt"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "temporary message"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let commit2_hash = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let commit2_hash = String::from_utf8_lossy(&commit2_hash.stdout)
            .trim()
            .to_string();

        // Should be able to push same message again after popping
        let result = manager.push_to_stack(
            "feature/temp2".to_string(),
            commit2_hash,
            "temporary message".to_string(),
            "main".to_string(),
        );

        assert!(result.is_ok());
    }
}
