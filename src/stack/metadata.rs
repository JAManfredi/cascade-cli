use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Metadata associated with a commit in the stack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitMetadata {
    /// The commit hash
    pub hash: String,
    /// Original commit message
    pub message: String,
    /// Stack entry ID this commit belongs to
    pub stack_entry_id: Uuid,
    /// Stack ID this commit belongs to
    pub stack_id: Uuid,
    /// Branch name where this commit lives
    pub branch: String,
    /// Dependent commit hashes (commits this one depends on)
    pub dependencies: Vec<String>,
    /// Commits that depend on this one
    pub dependents: Vec<String>,
    /// Whether this commit has been pushed to remote
    pub is_pushed: bool,
    /// Whether this commit is part of a submitted PR
    pub is_submitted: bool,
    /// Pull request ID if submitted
    pub pull_request_id: Option<String>,
    /// When this metadata was created
    pub created_at: DateTime<Utc>,
    /// When this metadata was last updated
    pub updated_at: DateTime<Utc>,
}

/// High-level metadata for a stack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackMetadata {
    /// Stack ID
    pub stack_id: Uuid,
    /// Stack name
    pub name: String,
    /// Stack description
    pub description: Option<String>,
    /// Base branch for this stack
    pub base_branch: String,
    /// Current active branch in the stack
    pub current_branch: Option<String>,
    /// Total number of commits in the stack
    pub total_commits: usize,
    /// Number of submitted commits
    pub submitted_commits: usize,
    /// Number of merged commits
    pub merged_commits: usize,
    /// All branches associated with this stack
    pub branches: Vec<String>,
    /// All commit hashes in this stack (in order)
    pub commit_hashes: Vec<String>,
    /// Whether this stack has conflicts
    pub has_conflicts: bool,
    /// Whether this stack is up to date with base
    pub is_up_to_date: bool,
    /// Last time this stack was synced
    pub last_sync: Option<DateTime<Utc>>,
    /// When this stack was created
    pub created_at: DateTime<Utc>,
    /// When this stack was last updated
    pub updated_at: DateTime<Utc>,
}

impl CommitMetadata {
    /// Create new commit metadata
    pub fn new(
        hash: String,
        message: String,
        stack_entry_id: Uuid,
        stack_id: Uuid,
        branch: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            hash,
            message,
            stack_entry_id,
            stack_id,
            branch,
            dependencies: Vec::new(),
            dependents: Vec::new(),
            is_pushed: false,
            is_submitted: false,
            pull_request_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a dependency (commit this one depends on)
    pub fn add_dependency(&mut self, commit_hash: String) {
        if !self.dependencies.contains(&commit_hash) {
            self.dependencies.push(commit_hash);
            self.updated_at = Utc::now();
        }
    }

    /// Add a dependent (commit that depends on this one)
    pub fn add_dependent(&mut self, commit_hash: String) {
        if !self.dependents.contains(&commit_hash) {
            self.dependents.push(commit_hash);
            self.updated_at = Utc::now();
        }
    }

    /// Mark as pushed to remote
    pub fn mark_pushed(&mut self) {
        self.is_pushed = true;
        self.updated_at = Utc::now();
    }

    /// Mark as submitted for review
    pub fn mark_submitted(&mut self, pull_request_id: String) {
        self.is_submitted = true;
        self.pull_request_id = Some(pull_request_id);
        self.updated_at = Utc::now();
    }

    /// Get a short version of the commit hash
    pub fn short_hash(&self) -> String {
        if self.hash.len() >= 8 {
            self.hash[..8].to_string()
        } else {
            self.hash.clone()
        }
    }
}

impl StackMetadata {
    /// Create new stack metadata
    pub fn new(
        stack_id: Uuid,
        name: String,
        base_branch: String,
        description: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            stack_id,
            name,
            description,
            base_branch,
            current_branch: None,
            total_commits: 0,
            submitted_commits: 0,
            merged_commits: 0,
            branches: Vec::new(),
            commit_hashes: Vec::new(),
            has_conflicts: false,
            is_up_to_date: true,
            last_sync: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update commit statistics
    pub fn update_stats(&mut self, total: usize, submitted: usize, merged: usize) {
        self.total_commits = total;
        self.submitted_commits = submitted;
        self.merged_commits = merged;
        self.updated_at = Utc::now();
    }

    /// Add a branch to this stack
    pub fn add_branch(&mut self, branch: String) {
        if !self.branches.contains(&branch) {
            self.branches.push(branch);
            self.updated_at = Utc::now();
        }
    }

    /// Remove a branch from this stack
    pub fn remove_branch(&mut self, branch: &str) {
        if let Some(pos) = self.branches.iter().position(|b| b == branch) {
            self.branches.remove(pos);
            self.updated_at = Utc::now();
        }
    }

    /// Set the current active branch
    pub fn set_current_branch(&mut self, branch: Option<String>) {
        self.current_branch = branch;
        self.updated_at = Utc::now();
    }

    /// Add a commit hash to the stack
    pub fn add_commit(&mut self, commit_hash: String) {
        if !self.commit_hashes.contains(&commit_hash) {
            self.commit_hashes.push(commit_hash);
            self.total_commits = self.commit_hashes.len();
            self.updated_at = Utc::now();
        }
    }

    /// Remove a commit hash from the stack
    pub fn remove_commit(&mut self, commit_hash: &str) {
        if let Some(pos) = self.commit_hashes.iter().position(|h| h == commit_hash) {
            self.commit_hashes.remove(pos);
            self.total_commits = self.commit_hashes.len();
            self.updated_at = Utc::now();
        }
    }

    /// Mark stack as having conflicts
    pub fn set_conflicts(&mut self, has_conflicts: bool) {
        self.has_conflicts = has_conflicts;
        self.updated_at = Utc::now();
    }

    /// Mark stack sync status
    pub fn set_up_to_date(&mut self, is_up_to_date: bool) {
        self.is_up_to_date = is_up_to_date;
        if is_up_to_date {
            self.last_sync = Some(Utc::now());
        }
        self.updated_at = Utc::now();
    }

    /// Get completion percentage (submitted/total)
    pub fn completion_percentage(&self) -> f64 {
        if self.total_commits == 0 {
            0.0
        } else {
            (self.submitted_commits as f64 / self.total_commits as f64) * 100.0
        }
    }

    /// Get merge percentage (merged/total)
    pub fn merge_percentage(&self) -> f64 {
        if self.total_commits == 0 {
            0.0
        } else {
            (self.merged_commits as f64 / self.total_commits as f64) * 100.0
        }
    }

    /// Check if the stack is complete (all commits submitted)
    pub fn is_complete(&self) -> bool {
        self.total_commits > 0 && self.submitted_commits == self.total_commits
    }

    /// Check if the stack is fully merged
    pub fn is_fully_merged(&self) -> bool {
        self.total_commits > 0 && self.merged_commits == self.total_commits
    }
}

/// Repository-wide stack metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMetadata {
    /// All stacks in this repository
    pub stacks: HashMap<Uuid, StackMetadata>,
    /// All commit metadata
    pub commits: HashMap<String, CommitMetadata>,
    /// Currently active stack ID
    pub active_stack_id: Option<Uuid>,
    /// Default base branch for new stacks
    pub default_base_branch: String,
    /// When this metadata was last updated
    pub updated_at: DateTime<Utc>,
}

impl RepositoryMetadata {
    /// Create new repository metadata
    pub fn new(default_base_branch: String) -> Self {
        Self {
            stacks: HashMap::new(),
            commits: HashMap::new(),
            active_stack_id: None,
            default_base_branch,
            updated_at: Utc::now(),
        }
    }

    /// Add a stack to the repository
    pub fn add_stack(&mut self, stack_metadata: StackMetadata) {
        self.stacks.insert(stack_metadata.stack_id, stack_metadata);
        self.updated_at = Utc::now();
    }

    /// Remove a stack from the repository
    pub fn remove_stack(&mut self, stack_id: &Uuid) -> Option<StackMetadata> {
        let removed = self.stacks.remove(stack_id);
        if removed.is_some() {
            // If this was the active stack, clear the active stack
            if self.active_stack_id == Some(*stack_id) {
                self.active_stack_id = None;
            }
            self.updated_at = Utc::now();
        }
        removed
    }

    /// Get a stack by ID
    pub fn get_stack(&self, stack_id: &Uuid) -> Option<&StackMetadata> {
        self.stacks.get(stack_id)
    }

    /// Get a mutable stack by ID
    pub fn get_stack_mut(&mut self, stack_id: &Uuid) -> Option<&mut StackMetadata> {
        self.stacks.get_mut(stack_id)
    }

    /// Set the active stack
    pub fn set_active_stack(&mut self, stack_id: Option<Uuid>) {
        self.active_stack_id = stack_id;
        self.updated_at = Utc::now();
    }

    /// Get the active stack
    pub fn get_active_stack(&self) -> Option<&StackMetadata> {
        self.active_stack_id.and_then(|id| self.stacks.get(&id))
    }

    /// Add commit metadata
    pub fn add_commit(&mut self, commit_metadata: CommitMetadata) {
        self.commits.insert(commit_metadata.hash.clone(), commit_metadata);
        self.updated_at = Utc::now();
    }

    /// Remove commit metadata
    pub fn remove_commit(&mut self, commit_hash: &str) -> Option<CommitMetadata> {
        let removed = self.commits.remove(commit_hash);
        if removed.is_some() {
            self.updated_at = Utc::now();
        }
        removed
    }

    /// Get commit metadata
    pub fn get_commit(&self, commit_hash: &str) -> Option<&CommitMetadata> {
        self.commits.get(commit_hash)
    }

    /// Get all stacks
    pub fn get_all_stacks(&self) -> Vec<&StackMetadata> {
        self.stacks.values().collect()
    }

    /// Get all commits for a stack
    pub fn get_stack_commits(&self, stack_id: &Uuid) -> Vec<&CommitMetadata> {
        self.commits.values()
            .filter(|commit| &commit.stack_id == stack_id)
            .collect()
    }

    /// Find stack by name
    pub fn find_stack_by_name(&self, name: &str) -> Option<&StackMetadata> {
        self.stacks.values().find(|stack| stack.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_metadata() {
        let stack_id = Uuid::new_v4();
        let entry_id = Uuid::new_v4();
        
        let mut commit = CommitMetadata::new(
            "abc123".to_string(),
            "Test commit".to_string(),
            entry_id,
            stack_id,
            "feature-branch".to_string(),
        );

        assert_eq!(commit.hash, "abc123");
        assert_eq!(commit.message, "Test commit");
        assert_eq!(commit.short_hash(), "abc123");
        assert!(!commit.is_pushed);
        assert!(!commit.is_submitted);

        commit.add_dependency("def456".to_string());
        assert_eq!(commit.dependencies, vec!["def456"]);

        commit.mark_pushed();
        assert!(commit.is_pushed);

        commit.mark_submitted("PR-123".to_string());
        assert!(commit.is_submitted);
        assert_eq!(commit.pull_request_id, Some("PR-123".to_string()));
    }

    #[test]
    fn test_stack_metadata() {
        let stack_id = Uuid::new_v4();
        let mut stack = StackMetadata::new(
            stack_id,
            "test-stack".to_string(),
            "main".to_string(),
            Some("Test stack".to_string()),
        );

        assert_eq!(stack.name, "test-stack");
        assert_eq!(stack.base_branch, "main");
        assert_eq!(stack.total_commits, 0);
        assert_eq!(stack.completion_percentage(), 0.0);

        stack.add_branch("feature-1".to_string());
        stack.add_commit("abc123".to_string());
        stack.update_stats(2, 1, 0);

        assert_eq!(stack.branches, vec!["feature-1"]);
        assert_eq!(stack.total_commits, 2);
        assert_eq!(stack.submitted_commits, 1);
        assert_eq!(stack.completion_percentage(), 50.0);
        assert!(!stack.is_complete());
        assert!(!stack.is_fully_merged());

        stack.update_stats(2, 2, 2);
        assert!(stack.is_complete());
        assert!(stack.is_fully_merged());
    }

    #[test]
    fn test_repository_metadata() {
        let mut repo = RepositoryMetadata::new("main".to_string());
        
        let stack_id = Uuid::new_v4();
        let stack = StackMetadata::new(
            stack_id,
            "test-stack".to_string(),
            "main".to_string(),
            None,
        );

        assert!(repo.get_active_stack().is_none());
        assert_eq!(repo.get_all_stacks().len(), 0);

        repo.add_stack(stack);
        assert_eq!(repo.get_all_stacks().len(), 1);
        assert!(repo.get_stack(&stack_id).is_some());

        repo.set_active_stack(Some(stack_id));
        assert!(repo.get_active_stack().is_some());
        assert_eq!(repo.get_active_stack().unwrap().stack_id, stack_id);

        let found = repo.find_stack_by_name("test-stack");
        assert!(found.is_some());
        assert_eq!(found.unwrap().stack_id, stack_id);
    }
} 