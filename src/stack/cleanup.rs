use crate::errors::{CascadeError, Result};
use crate::git::GitRepository;
use crate::stack::{Stack, StackManager};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::{debug, info, warn};

/// Information about a branch that can be cleaned up
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupCandidate {
    /// Branch name
    pub branch_name: String,
    /// Stack entry ID if this branch is part of a stack
    pub entry_id: Option<uuid::Uuid>,
    /// Stack ID if this branch is part of a stack
    pub stack_id: Option<uuid::Uuid>,
    /// Whether the branch is merged to the base branch
    pub is_merged: bool,
    /// Whether the branch has a remote tracking branch
    pub has_remote: bool,
    /// Whether the branch is the current branch
    pub is_current: bool,
    /// Reason this branch is a cleanup candidate
    pub reason: CleanupReason,
    /// Additional safety information
    pub safety_info: String,
}

impl CleanupCandidate {
    /// Get a human-readable string for the cleanup reason
    pub fn reason_to_string(&self) -> &str {
        match self.reason {
            CleanupReason::FullyMerged => "fully merged",
            CleanupReason::StackEntryMerged => "PR merged",
            CleanupReason::Stale => "stale",
            CleanupReason::Orphaned => "orphaned",
        }
    }
}

/// Reason why a branch is a cleanup candidate
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CleanupReason {
    /// Branch is fully merged to base branch
    FullyMerged,
    /// Stack entry was merged via PR
    StackEntryMerged,
    /// Branch is stale (old and no recent activity)
    Stale,
    /// Branch is a duplicate or orphaned branch
    Orphaned,
}

/// Options for cleanup operations
#[derive(Debug, Clone)]
pub struct CleanupOptions {
    /// Whether to run in dry-run mode (don't actually delete)
    pub dry_run: bool,
    /// Whether to skip confirmation prompts
    pub force: bool,
    /// Whether to include stale branches in cleanup
    pub include_stale: bool,
    /// Whether to cleanup remote tracking branches
    pub cleanup_remote: bool,
    /// Age threshold for stale branches (days)
    pub stale_threshold_days: u32,
    /// Whether to cleanup branches not in any stack
    pub cleanup_non_stack: bool,
}

/// Result of cleanup operation
#[derive(Debug, Clone)]
pub struct CleanupResult {
    /// Branches that were successfully cleaned up
    pub cleaned_branches: Vec<String>,
    /// Branches that failed to be cleaned up
    pub failed_branches: Vec<(String, String)>, // (branch_name, error)
    /// Branches that were skipped
    pub skipped_branches: Vec<(String, String)>, // (branch_name, reason)
    /// Total number of candidates found
    pub total_candidates: usize,
}

/// Manages cleanup operations for merged and stale branches
pub struct CleanupManager {
    stack_manager: StackManager,
    git_repo: GitRepository,
    options: CleanupOptions,
}

impl Default for CleanupOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            force: false,
            include_stale: false,
            cleanup_remote: false,
            stale_threshold_days: 30,
            cleanup_non_stack: false,
        }
    }
}

impl CleanupManager {
    /// Create a new cleanup manager
    pub fn new(
        stack_manager: StackManager,
        git_repo: GitRepository,
        options: CleanupOptions,
    ) -> Self {
        Self {
            stack_manager,
            git_repo,
            options,
        }
    }

    /// Find all branches that are candidates for cleanup
    pub fn find_cleanup_candidates(&self) -> Result<Vec<CleanupCandidate>> {
        debug!("Scanning for cleanup candidates...");

        let mut candidates = Vec::new();
        let all_branches = self.git_repo.list_branches()?;
        let current_branch = self.git_repo.get_current_branch().ok();

        // Get all stacks to identify stack branches
        let stacks = self.stack_manager.get_all_stacks_objects()?;
        let mut stack_branches = HashSet::new();
        let mut stack_branch_to_entry = std::collections::HashMap::new();

        for stack in &stacks {
            for entry in &stack.entries {
                stack_branches.insert(entry.branch.clone());
                stack_branch_to_entry.insert(entry.branch.clone(), (stack.id, entry.id));
            }
        }

        for branch_name in &all_branches {
            // Skip the current branch for safety
            if current_branch.as_ref() == Some(branch_name) {
                continue;
            }

            // Skip protected branches
            if self.is_protected_branch(branch_name) {
                continue;
            }

            let is_current = current_branch.as_ref() == Some(branch_name);
            let has_remote = self.git_repo.get_upstream_branch(branch_name)?.is_some();

            // Check if this branch is part of a stack
            let (stack_id, entry_id) =
                if let Some((stack_id, entry_id)) = stack_branch_to_entry.get(branch_name) {
                    (Some(*stack_id), Some(*entry_id))
                } else {
                    (None, None)
                };

            // Check different cleanup reasons
            if let Some(candidate) = self.evaluate_branch_for_cleanup(
                branch_name,
                stack_id,
                entry_id,
                is_current,
                has_remote,
                &stacks,
            )? {
                candidates.push(candidate);
            }
        }

        debug!("Found {} cleanup candidates", candidates.len());
        Ok(candidates)
    }

    /// Evaluate a single branch for cleanup
    fn evaluate_branch_for_cleanup(
        &self,
        branch_name: &str,
        stack_id: Option<uuid::Uuid>,
        entry_id: Option<uuid::Uuid>,
        is_current: bool,
        has_remote: bool,
        stacks: &[Stack],
    ) -> Result<Option<CleanupCandidate>> {
        // First check if branch is fully merged
        if let Ok(base_branch) = self.get_base_branch_for_branch(branch_name, stack_id, stacks) {
            if self.is_branch_merged_to_base(branch_name, &base_branch)? {
                return Ok(Some(CleanupCandidate {
                    branch_name: branch_name.to_string(),
                    entry_id,
                    stack_id,
                    is_merged: true,
                    has_remote,
                    is_current,
                    reason: CleanupReason::FullyMerged,
                    safety_info: format!("Branch fully merged to '{base_branch}'"),
                }));
            }
        }

        // Check if this is a stack entry that was merged via PR
        if let Some(entry_id) = entry_id {
            if let Some(stack_id) = stack_id {
                if self.is_stack_entry_merged(stack_id, entry_id)? {
                    return Ok(Some(CleanupCandidate {
                        branch_name: branch_name.to_string(),
                        entry_id: Some(entry_id),
                        stack_id: Some(stack_id),
                        is_merged: false, // Not git-merged, but PR-merged
                        has_remote,
                        is_current,
                        reason: CleanupReason::StackEntryMerged,
                        safety_info: "Stack entry was merged via pull request".to_string(),
                    }));
                }
            }
        }

        // Never clean up active stack branches - they're in use!
        if stack_id.is_some() {
            return Ok(None);
        }

        // Check for stale branches (if enabled) - only for NON-stack branches
        if self.options.include_stale {
            if let Some(candidate) =
                self.check_stale_branch(branch_name, stack_id, entry_id, has_remote, is_current)?
            {
                return Ok(Some(candidate));
            }
        }

        // Check for orphaned branches (if enabled)
        if self.options.cleanup_non_stack {
            if let Some(candidate) =
                self.check_orphaned_branch(branch_name, has_remote, is_current)?
            {
                return Ok(Some(candidate));
            }
        }

        Ok(None)
    }

    /// Check if a branch is stale based on last activity
    fn check_stale_branch(
        &self,
        branch_name: &str,
        stack_id: Option<uuid::Uuid>,
        entry_id: Option<uuid::Uuid>,
        _has_remote: bool,
        _is_current: bool,
    ) -> Result<Option<CleanupCandidate>> {
        let last_commit_age = self.get_branch_last_commit_age_days(branch_name)?;

        if last_commit_age > self.options.stale_threshold_days {
            return Ok(Some(CleanupCandidate {
                branch_name: branch_name.to_string(),
                entry_id,
                stack_id,
                is_merged: false,
                has_remote: _has_remote,
                is_current: _is_current,
                reason: CleanupReason::Stale,
                safety_info: format!("No activity for {last_commit_age} days"),
            }));
        }

        Ok(None)
    }

    /// Check if a branch is orphaned (not part of any stack)
    fn check_orphaned_branch(
        &self,
        _branch_name: &str,
        _has_remote: bool,
        _is_current: bool,
    ) -> Result<Option<CleanupCandidate>> {
        // For now, we'll be conservative and not automatically clean up non-stack branches
        // Users can explicitly enable this with --cleanup-non-stack
        Ok(None)
    }

    /// Perform cleanup based on the candidates found
    pub fn perform_cleanup(&mut self, candidates: &[CleanupCandidate]) -> Result<CleanupResult> {
        let mut result = CleanupResult {
            cleaned_branches: Vec::new(),
            failed_branches: Vec::new(),
            skipped_branches: Vec::new(),
            total_candidates: candidates.len(),
        };

        if candidates.is_empty() {
            info!("No cleanup candidates found");
            return Ok(result);
        }

        info!("Processing {} cleanup candidates", candidates.len());

        for candidate in candidates {
            match self.cleanup_single_branch(candidate) {
                Ok(true) => {
                    result.cleaned_branches.push(candidate.branch_name.clone());
                    info!("✅ Cleaned up branch: {}", candidate.branch_name);
                }
                Ok(false) => {
                    result.skipped_branches.push((
                        candidate.branch_name.clone(),
                        "Skipped by user or safety check".to_string(),
                    ));
                    debug!("⏭️  Skipped branch: {}", candidate.branch_name);
                }
                Err(e) => {
                    result
                        .failed_branches
                        .push((candidate.branch_name.clone(), e.to_string()));
                    warn!(
                        "❌ Failed to clean up branch {}: {}",
                        candidate.branch_name, e
                    );
                }
            }
        }

        Ok(result)
    }

    /// Clean up a single branch
    fn cleanup_single_branch(&mut self, candidate: &CleanupCandidate) -> Result<bool> {
        debug!(
            "Cleaning up branch: {} ({:?})",
            candidate.branch_name, candidate.reason
        );

        // Safety check: don't delete current branch
        if candidate.is_current {
            return Ok(false);
        }

        // In dry-run mode, just report what would be done
        if self.options.dry_run {
            info!("DRY RUN: Would delete branch '{}'", candidate.branch_name);
            return Ok(true);
        }

        // Delete the branch
        match candidate.reason {
            CleanupReason::FullyMerged | CleanupReason::StackEntryMerged => {
                // Safe to delete merged branches
                self.git_repo.delete_branch(&candidate.branch_name)?;
            }
            CleanupReason::Stale | CleanupReason::Orphaned => {
                // Use unsafe delete for stale/orphaned branches (they might not be merged)
                self.git_repo.delete_branch_unsafe(&candidate.branch_name)?;
            }
        }

        // Remove from stack metadata if it's a stack branch
        if let (Some(stack_id), Some(entry_id)) = (candidate.stack_id, candidate.entry_id) {
            self.remove_entry_from_stack(stack_id, entry_id)?;
        }

        Ok(true)
    }

    /// Remove an entry from stack metadata
    fn remove_entry_from_stack(
        &mut self,
        stack_id: uuid::Uuid,
        entry_id: uuid::Uuid,
    ) -> Result<()> {
        debug!("Removing entry {} from stack {}", entry_id, stack_id);

        if let Some(stack) = self.stack_manager.get_stack_mut(&stack_id) {
            stack.entries.retain(|entry| entry.id != entry_id);

            // If the stack is now empty, we might want to remove it
            if stack.entries.is_empty() {
                info!("Stack '{}' is now empty after cleanup", stack.name);
            }
        }

        Ok(())
    }

    /// Check if a branch is merged to its base branch
    fn is_branch_merged_to_base(&self, branch_name: &str, base_branch: &str) -> Result<bool> {
        // Get the commits between base and the branch
        match self.git_repo.get_commits_between(base_branch, branch_name) {
            Ok(commits) => Ok(commits.is_empty()),
            Err(_) => {
                // If we can't determine, assume not merged for safety
                Ok(false)
            }
        }
    }

    /// Check if a stack entry was merged via PR
    fn is_stack_entry_merged(&self, stack_id: uuid::Uuid, entry_id: uuid::Uuid) -> Result<bool> {
        // Check if the entry is marked as submitted and has been merged in the stack metadata
        if let Some(stack) = self.stack_manager.get_stack(&stack_id) {
            if let Some(entry) = stack.entries.iter().find(|e| e.id == entry_id) {
                // For now, consider it merged if it's submitted
                // In a full implementation, we'd check the PR status via Bitbucket API
                return Ok(entry.is_submitted);
            }
        }
        Ok(false)
    }

    /// Get the base branch for a given branch
    fn get_base_branch_for_branch(
        &self,
        _branch_name: &str,
        stack_id: Option<uuid::Uuid>,
        stacks: &[Stack],
    ) -> Result<String> {
        if let Some(stack_id) = stack_id {
            if let Some(stack) = stacks.iter().find(|s| s.id == stack_id) {
                return Ok(stack.base_branch.clone());
            }
        }

        // Default to main/master if not in a stack
        let main_branches = ["main", "master", "develop"];
        for branch in &main_branches {
            if self.git_repo.branch_exists(branch) {
                return Ok(branch.to_string());
            }
        }

        Err(CascadeError::config(
            "Could not determine base branch".to_string(),
        ))
    }

    /// Check if a branch is protected (shouldn't be deleted)
    fn is_protected_branch(&self, branch_name: &str) -> bool {
        let protected_branches = [
            "main",
            "master",
            "develop",
            "development",
            "staging",
            "production",
            "release",
        ];

        protected_branches.contains(&branch_name)
    }

    /// Get the age of the last commit on a branch in days
    fn get_branch_last_commit_age_days(&self, branch_name: &str) -> Result<u32> {
        let commit_hash = self.git_repo.get_branch_commit_hash(branch_name)?;
        let commit = self.git_repo.get_commit(&commit_hash)?;

        let now = std::time::SystemTime::now();
        let commit_time =
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(commit.time().seconds() as u64);

        let age = now
            .duration_since(commit_time)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0));

        Ok((age.as_secs() / 86400) as u32) // Convert to days
    }

    /// Get cleanup statistics
    pub fn get_cleanup_stats(&self) -> Result<CleanupStats> {
        let candidates = self.find_cleanup_candidates()?;

        let mut stats = CleanupStats {
            total_branches: self.git_repo.list_branches()?.len(),
            fully_merged: 0,
            stack_entry_merged: 0,
            stale: 0,
            orphaned: 0,
            protected: 0,
        };

        for candidate in &candidates {
            match candidate.reason {
                CleanupReason::FullyMerged => stats.fully_merged += 1,
                CleanupReason::StackEntryMerged => stats.stack_entry_merged += 1,
                CleanupReason::Stale => stats.stale += 1,
                CleanupReason::Orphaned => stats.orphaned += 1,
            }
        }

        // Count protected branches
        let all_branches = self.git_repo.list_branches()?;
        stats.protected = all_branches
            .iter()
            .filter(|branch| self.is_protected_branch(branch))
            .count();

        Ok(stats)
    }
}

/// Statistics about cleanup candidates
#[derive(Debug, Clone)]
pub struct CleanupStats {
    pub total_branches: usize,
    pub fully_merged: usize,
    pub stack_entry_merged: usize,
    pub stale: usize,
    pub orphaned: usize,
    pub protected: usize,
}

impl CleanupStats {
    pub fn cleanup_candidates(&self) -> usize {
        self.fully_merged + self.stack_entry_merged + self.stale + self.orphaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repository
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_cleanup_reason_serialization() {
        let reason = CleanupReason::FullyMerged;
        let serialized = serde_json::to_string(&reason).unwrap();
        let deserialized: CleanupReason = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reason, deserialized);
    }

    #[test]
    fn test_cleanup_options_default() {
        let options = CleanupOptions::default();
        assert!(!options.dry_run);
        assert!(!options.force);
        assert!(!options.include_stale);
        assert_eq!(options.stale_threshold_days, 30);
    }

    #[test]
    fn test_protected_branches() {
        let (_temp_dir, repo_path) = create_test_repo();
        let git_repo = crate::git::GitRepository::open(&repo_path).unwrap();
        let stack_manager = crate::stack::StackManager::new(&repo_path).unwrap();
        let options = CleanupOptions::default();

        let cleanup_manager = CleanupManager::new(stack_manager, git_repo, options);

        assert!(cleanup_manager.is_protected_branch("main"));
        assert!(cleanup_manager.is_protected_branch("master"));
        assert!(cleanup_manager.is_protected_branch("develop"));
        assert!(!cleanup_manager.is_protected_branch("feature-branch"));
    }

    #[test]
    fn test_cleanup_stats() {
        let stats = CleanupStats {
            total_branches: 10,
            fully_merged: 3,
            stack_entry_merged: 2,
            stale: 1,
            orphaned: 0,
            protected: 4,
        };

        assert_eq!(stats.cleanup_candidates(), 6);
    }
}
