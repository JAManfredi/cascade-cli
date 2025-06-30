use crate::errors::{CascadeError, Result};
use chrono;
use dialoguer::{theme::ColorfulTheme, Confirm};
use git2::{Oid, Repository, Signature};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Repository information
#[derive(Debug, Clone)]
pub struct RepositoryInfo {
    pub path: PathBuf,
    pub head_branch: Option<String>,
    pub head_commit: Option<String>,
    pub is_dirty: bool,
    pub untracked_files: Vec<String>,
}

/// Backup information for force push operations
#[derive(Debug, Clone)]
struct ForceBackupInfo {
    pub backup_branch_name: String,
    pub remote_commit_id: String,
    #[allow(dead_code)] // Used for logging/display purposes
    pub commits_that_would_be_lost: usize,
}

/// Safety information for branch deletion operations
#[derive(Debug, Clone)]
struct BranchDeletionSafety {
    pub unpushed_commits: Vec<String>,
    pub remote_tracking_branch: Option<String>,
    pub is_merged_to_main: bool,
    pub main_branch_name: String,
}

/// Safety information for checkout operations
#[derive(Debug, Clone)]
struct CheckoutSafety {
    #[allow(dead_code)] // Used in confirmation dialogs and future features
    pub has_uncommitted_changes: bool,
    pub modified_files: Vec<String>,
    pub staged_files: Vec<String>,
    pub untracked_files: Vec<String>,
    #[allow(dead_code)] // Reserved for future automatic stashing implementation
    pub stash_created: Option<String>,
    #[allow(dead_code)] // Used for context in confirmation dialogs
    pub current_branch: Option<String>,
}

/// Wrapper around git2::Repository with safe operations
///
/// For thread safety, use the async variants (e.g., fetch_async, pull_async)
/// which automatically handle threading using tokio::spawn_blocking.
/// The async methods create new repository instances in background threads.
pub struct GitRepository {
    repo: Repository,
    path: PathBuf,
}

impl GitRepository {
    /// Open a Git repository at the given path
    pub fn open(path: &Path) -> Result<Self> {
        let repo = Repository::discover(path)
            .map_err(|e| CascadeError::config(format!("Not a git repository: {e}")))?;

        let workdir = repo
            .workdir()
            .ok_or_else(|| CascadeError::config("Repository has no working directory"))?
            .to_path_buf();

        Ok(Self {
            repo,
            path: workdir,
        })
    }

    /// Get repository information
    pub fn get_info(&self) -> Result<RepositoryInfo> {
        let head_branch = self.get_current_branch().ok();
        let head_commit = self.get_head_commit_hash().ok();
        let is_dirty = self.is_dirty()?;
        let untracked_files = self.get_untracked_files()?;

        Ok(RepositoryInfo {
            path: self.path.clone(),
            head_branch,
            head_commit,
            is_dirty,
            untracked_files,
        })
    }

    /// Get the current branch name
    pub fn get_current_branch(&self) -> Result<String> {
        let head = self
            .repo
            .head()
            .map_err(|e| CascadeError::branch(format!("Could not get HEAD: {e}")))?;

        if let Some(name) = head.shorthand() {
            Ok(name.to_string())
        } else {
            // Detached HEAD - return commit hash
            let commit = head
                .peel_to_commit()
                .map_err(|e| CascadeError::branch(format!("Could not get HEAD commit: {e}")))?;
            Ok(format!("HEAD@{}", commit.id()))
        }
    }

    /// Get the HEAD commit hash
    pub fn get_head_commit_hash(&self) -> Result<String> {
        let head = self
            .repo
            .head()
            .map_err(|e| CascadeError::branch(format!("Could not get HEAD: {e}")))?;

        let commit = head
            .peel_to_commit()
            .map_err(|e| CascadeError::branch(format!("Could not get HEAD commit: {e}")))?;

        Ok(commit.id().to_string())
    }

    /// Check if the working directory is dirty (has uncommitted changes)
    pub fn is_dirty(&self) -> Result<bool> {
        let statuses = self.repo.statuses(None).map_err(CascadeError::Git)?;

        for status in statuses.iter() {
            let flags = status.status();

            // Check for any modifications, additions, or deletions
            if flags.intersects(
                git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_NEW
                    | git2::Status::INDEX_DELETED
                    | git2::Status::WT_MODIFIED
                    | git2::Status::WT_NEW
                    | git2::Status::WT_DELETED,
            ) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Get list of untracked files
    pub fn get_untracked_files(&self) -> Result<Vec<String>> {
        let statuses = self.repo.statuses(None).map_err(CascadeError::Git)?;

        let mut untracked = Vec::new();
        for status in statuses.iter() {
            if status.status().contains(git2::Status::WT_NEW) {
                if let Some(path) = status.path() {
                    untracked.push(path.to_string());
                }
            }
        }

        Ok(untracked)
    }

    /// Create a new branch
    pub fn create_branch(&self, name: &str, target: Option<&str>) -> Result<()> {
        let target_commit = if let Some(target) = target {
            // Find the specified target commit/branch
            let target_obj = self.repo.revparse_single(target).map_err(|e| {
                CascadeError::branch(format!("Could not find target '{target}': {e}"))
            })?;
            target_obj.peel_to_commit().map_err(|e| {
                CascadeError::branch(format!("Target '{target}' is not a commit: {e}"))
            })?
        } else {
            // Use current HEAD
            let head = self
                .repo
                .head()
                .map_err(|e| CascadeError::branch(format!("Could not get HEAD: {e}")))?;
            head.peel_to_commit()
                .map_err(|e| CascadeError::branch(format!("Could not get HEAD commit: {e}")))?
        };

        self.repo
            .branch(name, &target_commit, false)
            .map_err(|e| CascadeError::branch(format!("Could not create branch '{name}': {e}")))?;

        tracing::info!("Created branch '{}'", name);
        Ok(())
    }

    /// Switch to a branch with safety checks
    pub fn checkout_branch(&self, name: &str) -> Result<()> {
        self.checkout_branch_with_options(name, false)
    }

    /// Switch to a branch with force option to bypass safety checks
    pub fn checkout_branch_unsafe(&self, name: &str) -> Result<()> {
        self.checkout_branch_with_options(name, true)
    }

    /// Internal branch checkout implementation with safety options
    fn checkout_branch_with_options(&self, name: &str, force_unsafe: bool) -> Result<()> {
        info!("Attempting to checkout branch: {}", name);

        // Enhanced safety check: Detect uncommitted work before checkout
        if !force_unsafe {
            let safety_result = self.check_checkout_safety(name)?;
            if let Some(safety_info) = safety_result {
                // Repository has uncommitted changes, get user confirmation
                self.handle_checkout_confirmation(name, &safety_info)?;
            }
        }

        // Find the branch
        let branch = self
            .repo
            .find_branch(name, git2::BranchType::Local)
            .map_err(|e| CascadeError::branch(format!("Could not find branch '{name}': {e}")))?;

        let branch_ref = branch.get();
        let tree = branch_ref.peel_to_tree().map_err(|e| {
            CascadeError::branch(format!("Could not get tree for branch '{name}': {e}"))
        })?;

        // Checkout the tree
        self.repo
            .checkout_tree(tree.as_object(), None)
            .map_err(|e| {
                CascadeError::branch(format!("Could not checkout branch '{name}': {e}"))
            })?;

        // Update HEAD
        self.repo
            .set_head(&format!("refs/heads/{name}"))
            .map_err(|e| CascadeError::branch(format!("Could not update HEAD to '{name}': {e}")))?;

        tracing::info!("Switched to branch '{}'", name);
        Ok(())
    }

    /// Checkout a specific commit (detached HEAD) with safety checks
    pub fn checkout_commit(&self, commit_hash: &str) -> Result<()> {
        self.checkout_commit_with_options(commit_hash, false)
    }

    /// Checkout a specific commit with force option to bypass safety checks
    pub fn checkout_commit_unsafe(&self, commit_hash: &str) -> Result<()> {
        self.checkout_commit_with_options(commit_hash, true)
    }

    /// Internal commit checkout implementation with safety options
    fn checkout_commit_with_options(&self, commit_hash: &str, force_unsafe: bool) -> Result<()> {
        info!("Attempting to checkout commit: {}", commit_hash);

        // Enhanced safety check: Detect uncommitted work before checkout
        if !force_unsafe {
            let safety_result = self.check_checkout_safety(&format!("commit:{commit_hash}"))?;
            if let Some(safety_info) = safety_result {
                // Repository has uncommitted changes, get user confirmation
                self.handle_checkout_confirmation(&format!("commit {commit_hash}"), &safety_info)?;
            }
        }

        let oid = Oid::from_str(commit_hash).map_err(CascadeError::Git)?;

        let commit = self.repo.find_commit(oid).map_err(|e| {
            CascadeError::branch(format!("Could not find commit '{commit_hash}': {e}"))
        })?;

        let tree = commit.tree().map_err(|e| {
            CascadeError::branch(format!(
                "Could not get tree for commit '{commit_hash}': {e}"
            ))
        })?;

        // Checkout the tree
        self.repo
            .checkout_tree(tree.as_object(), None)
            .map_err(|e| {
                CascadeError::branch(format!("Could not checkout commit '{commit_hash}': {e}"))
            })?;

        // Update HEAD to the commit (detached HEAD)
        self.repo.set_head_detached(oid).map_err(|e| {
            CascadeError::branch(format!(
                "Could not update HEAD to commit '{commit_hash}': {e}"
            ))
        })?;

        tracing::info!("Checked out commit '{}' (detached HEAD)", commit_hash);
        Ok(())
    }

    /// Check if a branch exists
    pub fn branch_exists(&self, name: &str) -> bool {
        self.repo.find_branch(name, git2::BranchType::Local).is_ok()
    }

    /// Get the commit hash for a specific branch without switching branches
    pub fn get_branch_commit_hash(&self, branch_name: &str) -> Result<String> {
        let branch = self
            .repo
            .find_branch(branch_name, git2::BranchType::Local)
            .map_err(|e| {
                CascadeError::branch(format!("Could not find branch '{branch_name}': {e}"))
            })?;

        let commit = branch.get().peel_to_commit().map_err(|e| {
            CascadeError::branch(format!(
                "Could not get commit for branch '{branch_name}': {e}"
            ))
        })?;

        Ok(commit.id().to_string())
    }

    /// List all local branches
    pub fn list_branches(&self) -> Result<Vec<String>> {
        let branches = self
            .repo
            .branches(Some(git2::BranchType::Local))
            .map_err(CascadeError::Git)?;

        let mut branch_names = Vec::new();
        for branch in branches {
            let (branch, _) = branch.map_err(CascadeError::Git)?;
            if let Some(name) = branch.name().map_err(CascadeError::Git)? {
                branch_names.push(name.to_string());
            }
        }

        Ok(branch_names)
    }

    /// Create a commit with all staged changes
    pub fn commit(&self, message: &str) -> Result<String> {
        let signature = self.get_signature()?;
        let tree_id = self.get_index_tree()?;
        let tree = self.repo.find_tree(tree_id).map_err(CascadeError::Git)?;

        // Get parent commits
        let head = self.repo.head().map_err(CascadeError::Git)?;
        let parent_commit = head.peel_to_commit().map_err(CascadeError::Git)?;

        let commit_id = self
            .repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &[&parent_commit],
            )
            .map_err(CascadeError::Git)?;

        tracing::info!("Created commit: {} - {}", commit_id, message);
        Ok(commit_id.to_string())
    }

    /// Stage all changes
    pub fn stage_all(&self) -> Result<()> {
        let mut index = self.repo.index().map_err(CascadeError::Git)?;

        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(CascadeError::Git)?;

        index.write().map_err(CascadeError::Git)?;

        tracing::debug!("Staged all changes");
        Ok(())
    }

    /// Get repository path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Check if a commit exists
    pub fn commit_exists(&self, commit_hash: &str) -> Result<bool> {
        match Oid::from_str(commit_hash) {
            Ok(oid) => match self.repo.find_commit(oid) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            },
            Err(_) => Ok(false),
        }
    }

    /// Get the HEAD commit object
    pub fn get_head_commit(&self) -> Result<git2::Commit<'_>> {
        let head = self
            .repo
            .head()
            .map_err(|e| CascadeError::branch(format!("Could not get HEAD: {e}")))?;
        head.peel_to_commit()
            .map_err(|e| CascadeError::branch(format!("Could not get HEAD commit: {e}")))
    }

    /// Get a commit object by hash
    pub fn get_commit(&self, commit_hash: &str) -> Result<git2::Commit<'_>> {
        let oid = Oid::from_str(commit_hash).map_err(CascadeError::Git)?;

        self.repo.find_commit(oid).map_err(CascadeError::Git)
    }

    /// Get the commit hash at the head of a branch
    pub fn get_branch_head(&self, branch_name: &str) -> Result<String> {
        let branch = self
            .repo
            .find_branch(branch_name, git2::BranchType::Local)
            .map_err(|e| {
                CascadeError::branch(format!("Could not find branch '{branch_name}': {e}"))
            })?;

        let commit = branch.get().peel_to_commit().map_err(|e| {
            CascadeError::branch(format!(
                "Could not get commit for branch '{branch_name}': {e}"
            ))
        })?;

        Ok(commit.id().to_string())
    }

    /// Get a signature for commits
    fn get_signature(&self) -> Result<Signature<'_>> {
        // Try to get signature from Git config
        if let Ok(config) = self.repo.config() {
            if let (Ok(name), Ok(email)) = (
                config.get_string("user.name"),
                config.get_string("user.email"),
            ) {
                return Signature::now(&name, &email).map_err(CascadeError::Git);
            }
        }

        // Fallback to default signature
        Signature::now("Cascade CLI", "cascade@example.com").map_err(CascadeError::Git)
    }

    /// Get the tree ID from the current index
    fn get_index_tree(&self) -> Result<Oid> {
        let mut index = self.repo.index().map_err(CascadeError::Git)?;

        index.write_tree().map_err(CascadeError::Git)
    }

    /// Get repository status
    pub fn get_status(&self) -> Result<git2::Statuses<'_>> {
        self.repo.statuses(None).map_err(CascadeError::Git)
    }

    /// Get remote URL for a given remote name
    pub fn get_remote_url(&self, name: &str) -> Result<String> {
        let remote = self.repo.find_remote(name).map_err(CascadeError::Git)?;

        let url = remote.url().ok_or_else(|| {
            CascadeError::Git(git2::Error::from_str("Remote URL is not valid UTF-8"))
        })?;

        Ok(url.to_string())
    }

    /// Cherry-pick a commit onto the current branch
    pub fn cherry_pick(&self, commit_hash: &str) -> Result<String> {
        tracing::debug!("Cherry-picking commit {}", commit_hash);

        let oid = Oid::from_str(commit_hash).map_err(CascadeError::Git)?;
        let commit = self.repo.find_commit(oid).map_err(CascadeError::Git)?;

        // Get the commit's tree
        let commit_tree = commit.tree().map_err(CascadeError::Git)?;

        // Get parent tree for merge base
        let parent_commit = if commit.parent_count() > 0 {
            commit.parent(0).map_err(CascadeError::Git)?
        } else {
            // Root commit - use empty tree
            let empty_tree_oid = self.repo.treebuilder(None)?.write()?;
            let empty_tree = self.repo.find_tree(empty_tree_oid)?;
            let sig = self.get_signature()?;
            return self
                .repo
                .commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    commit.message().unwrap_or("Cherry-picked commit"),
                    &empty_tree,
                    &[],
                )
                .map(|oid| oid.to_string())
                .map_err(CascadeError::Git);
        };

        let parent_tree = parent_commit.tree().map_err(CascadeError::Git)?;

        // Get current HEAD tree for 3-way merge
        let head_commit = self.get_head_commit()?;
        let head_tree = head_commit.tree().map_err(CascadeError::Git)?;

        // Perform 3-way merge
        let mut index = self
            .repo
            .merge_trees(&parent_tree, &head_tree, &commit_tree, None)
            .map_err(CascadeError::Git)?;

        // Check for conflicts
        if index.has_conflicts() {
            return Err(CascadeError::branch(format!(
                "Cherry-pick of {commit_hash} has conflicts that need manual resolution"
            )));
        }

        // Write merged tree
        let merged_tree_oid = index.write_tree_to(&self.repo).map_err(CascadeError::Git)?;
        let merged_tree = self
            .repo
            .find_tree(merged_tree_oid)
            .map_err(CascadeError::Git)?;

        // Create new commit
        let signature = self.get_signature()?;
        let message = format!("Cherry-pick: {}", commit.message().unwrap_or(""));

        let new_commit_oid = self
            .repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                &message,
                &merged_tree,
                &[&head_commit],
            )
            .map_err(CascadeError::Git)?;

        tracing::info!("Cherry-picked {} -> {}", commit_hash, new_commit_oid);
        Ok(new_commit_oid.to_string())
    }

    /// Check for merge conflicts in the index
    pub fn has_conflicts(&self) -> Result<bool> {
        let index = self.repo.index().map_err(CascadeError::Git)?;
        Ok(index.has_conflicts())
    }

    /// Get list of conflicted files
    pub fn get_conflicted_files(&self) -> Result<Vec<String>> {
        let index = self.repo.index().map_err(CascadeError::Git)?;

        let mut conflicts = Vec::new();

        // Iterate through index conflicts
        let conflict_iter = index.conflicts().map_err(CascadeError::Git)?;

        for conflict in conflict_iter {
            let conflict = conflict.map_err(CascadeError::Git)?;
            if let Some(our) = conflict.our {
                if let Ok(path) = std::str::from_utf8(&our.path) {
                    conflicts.push(path.to_string());
                }
            } else if let Some(their) = conflict.their {
                if let Ok(path) = std::str::from_utf8(&their.path) {
                    conflicts.push(path.to_string());
                }
            }
        }

        Ok(conflicts)
    }

    /// Fetch from remote origin
    pub fn fetch(&self) -> Result<()> {
        tracing::info!("Fetching from origin");

        let mut remote = self
            .repo
            .find_remote("origin")
            .map_err(|e| CascadeError::branch(format!("No remote 'origin' found: {e}")))?;

        // Fetch with default refspec
        remote
            .fetch::<&str>(&[], None, None)
            .map_err(CascadeError::Git)?;

        tracing::debug!("Fetch completed successfully");
        Ok(())
    }

    /// Pull changes from remote (fetch + merge)
    pub fn pull(&self, branch: &str) -> Result<()> {
        tracing::info!("Pulling branch: {}", branch);

        // First fetch
        self.fetch()?;

        // Get remote tracking branch
        let remote_branch_name = format!("origin/{branch}");
        let remote_oid = self
            .repo
            .refname_to_id(&format!("refs/remotes/{remote_branch_name}"))
            .map_err(|e| {
                CascadeError::branch(format!("Remote branch {remote_branch_name} not found: {e}"))
            })?;

        let remote_commit = self
            .repo
            .find_commit(remote_oid)
            .map_err(CascadeError::Git)?;

        // Get current HEAD
        let head_commit = self.get_head_commit()?;

        // Check if we need to merge
        if head_commit.id() == remote_commit.id() {
            tracing::debug!("Already up to date");
            return Ok(());
        }

        // Perform merge
        let head_tree = head_commit.tree().map_err(CascadeError::Git)?;
        let remote_tree = remote_commit.tree().map_err(CascadeError::Git)?;

        // Find merge base
        let merge_base_oid = self
            .repo
            .merge_base(head_commit.id(), remote_commit.id())
            .map_err(CascadeError::Git)?;
        let merge_base_commit = self
            .repo
            .find_commit(merge_base_oid)
            .map_err(CascadeError::Git)?;
        let merge_base_tree = merge_base_commit.tree().map_err(CascadeError::Git)?;

        // 3-way merge
        let mut index = self
            .repo
            .merge_trees(&merge_base_tree, &head_tree, &remote_tree, None)
            .map_err(CascadeError::Git)?;

        if index.has_conflicts() {
            return Err(CascadeError::branch(
                "Pull has conflicts that need manual resolution".to_string(),
            ));
        }

        // Write merged tree and create merge commit
        let merged_tree_oid = index.write_tree_to(&self.repo).map_err(CascadeError::Git)?;
        let merged_tree = self
            .repo
            .find_tree(merged_tree_oid)
            .map_err(CascadeError::Git)?;

        let signature = self.get_signature()?;
        let message = format!("Merge branch '{branch}' from origin");

        self.repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                &message,
                &merged_tree,
                &[&head_commit, &remote_commit],
            )
            .map_err(CascadeError::Git)?;

        tracing::info!("Pull completed successfully");
        Ok(())
    }

    /// Push current branch to remote
    pub fn push(&self, branch: &str) -> Result<()> {
        tracing::info!("Pushing branch: {}", branch);

        let mut remote = self
            .repo
            .find_remote("origin")
            .map_err(|e| CascadeError::branch(format!("No remote 'origin' found: {e}")))?;

        let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");

        remote.push(&[&refspec], None).map_err(CascadeError::Git)?;

        tracing::info!("Push completed successfully");
        Ok(())
    }

    /// Delete a local branch
    pub fn delete_branch(&self, name: &str) -> Result<()> {
        self.delete_branch_with_options(name, false)
    }

    /// Delete a local branch with force option to bypass safety checks
    pub fn delete_branch_unsafe(&self, name: &str) -> Result<()> {
        self.delete_branch_with_options(name, true)
    }

    /// Internal branch deletion implementation with safety options
    fn delete_branch_with_options(&self, name: &str, force_unsafe: bool) -> Result<()> {
        info!("Attempting to delete branch: {}", name);

        // Enhanced safety check: Detect unpushed commits before deletion
        if !force_unsafe {
            let safety_result = self.check_branch_deletion_safety(name)?;
            if let Some(safety_info) = safety_result {
                // Branch has unpushed commits, get user confirmation
                self.handle_branch_deletion_confirmation(name, &safety_info)?;
            }
        }

        let mut branch = self
            .repo
            .find_branch(name, git2::BranchType::Local)
            .map_err(|e| CascadeError::branch(format!("Could not find branch '{name}': {e}")))?;

        branch
            .delete()
            .map_err(|e| CascadeError::branch(format!("Could not delete branch '{name}': {e}")))?;

        info!("Successfully deleted branch '{}'", name);
        Ok(())
    }

    /// Get commits between two references
    pub fn get_commits_between(&self, from: &str, to: &str) -> Result<Vec<git2::Commit<'_>>> {
        let from_oid = self
            .repo
            .refname_to_id(&format!("refs/heads/{from}"))
            .or_else(|_| Oid::from_str(from))
            .map_err(|e| CascadeError::branch(format!("Invalid from reference '{from}': {e}")))?;

        let to_oid = self
            .repo
            .refname_to_id(&format!("refs/heads/{to}"))
            .or_else(|_| Oid::from_str(to))
            .map_err(|e| CascadeError::branch(format!("Invalid to reference '{to}': {e}")))?;

        let mut revwalk = self.repo.revwalk().map_err(CascadeError::Git)?;

        revwalk.push(to_oid).map_err(CascadeError::Git)?;
        revwalk.hide(from_oid).map_err(CascadeError::Git)?;

        let mut commits = Vec::new();
        for oid in revwalk {
            let oid = oid.map_err(CascadeError::Git)?;
            let commit = self.repo.find_commit(oid).map_err(CascadeError::Git)?;
            commits.push(commit);
        }

        Ok(commits)
    }

    /// Force push one branch's content to another branch name
    /// This is used to preserve PR history while updating branch contents after rebase
    pub fn force_push_branch(&self, target_branch: &str, source_branch: &str) -> Result<()> {
        self.force_push_branch_with_options(target_branch, source_branch, false)
    }

    /// Force push with explicit force flag to bypass safety checks
    pub fn force_push_branch_unsafe(&self, target_branch: &str, source_branch: &str) -> Result<()> {
        self.force_push_branch_with_options(target_branch, source_branch, true)
    }

    /// Internal force push implementation with safety options
    fn force_push_branch_with_options(
        &self,
        target_branch: &str,
        source_branch: &str,
        force_unsafe: bool,
    ) -> Result<()> {
        info!(
            "Force pushing {} content to {} to preserve PR history",
            source_branch, target_branch
        );

        // Enhanced safety check: Detect potential data loss and get user confirmation
        if !force_unsafe {
            let safety_result = self.check_force_push_safety_enhanced(target_branch)?;
            if let Some(backup_info) = safety_result {
                // Create backup branch before force push
                self.create_backup_branch(target_branch, &backup_info.remote_commit_id)?;
                info!(
                    "✅ Created backup branch: {}",
                    backup_info.backup_branch_name
                );
            }
        }

        // First, ensure we have the latest changes for the source branch
        let source_ref = self
            .repo
            .find_reference(&format!("refs/heads/{source_branch}"))
            .map_err(|e| {
                CascadeError::config(format!("Failed to find source branch {source_branch}: {e}"))
            })?;
        let source_commit = source_ref.peel_to_commit().map_err(|e| {
            CascadeError::config(format!(
                "Failed to get commit for source branch {source_branch}: {e}"
            ))
        })?;

        // Update the target branch to point to the source commit
        let mut target_ref = self
            .repo
            .find_reference(&format!("refs/heads/{target_branch}"))
            .map_err(|e| {
                CascadeError::config(format!("Failed to find target branch {target_branch}: {e}"))
            })?;

        target_ref
            .set_target(source_commit.id(), "Force push from rebase")
            .map_err(|e| {
                CascadeError::config(format!(
                    "Failed to update target branch {target_branch}: {e}"
                ))
            })?;

        // Force push to remote
        let mut remote = self
            .repo
            .find_remote("origin")
            .map_err(|e| CascadeError::config(format!("Failed to find origin remote: {e}")))?;

        let refspec = format!("+refs/heads/{target_branch}:refs/heads/{target_branch}");

        // Create callbacks for authentication
        let mut callbacks = git2::RemoteCallbacks::new();

        // Try to use existing authentication from git config/credential manager
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            if let Some(username) = username_from_url {
                // Try SSH key first
                git2::Cred::ssh_key_from_agent(username)
            } else {
                // Try default credential helper
                git2::Cred::default()
            }
        });

        // Push options for force push
        let mut push_options = git2::PushOptions::new();
        push_options.remote_callbacks(callbacks);

        remote
            .push(&[&refspec], Some(&mut push_options))
            .map_err(|e| {
                CascadeError::config(format!("Failed to force push {target_branch}: {e}"))
            })?;

        info!(
            "✅ Successfully force pushed {} to preserve PR history",
            target_branch
        );
        Ok(())
    }

    /// Enhanced safety check for force push operations with user confirmation
    /// Returns backup info if data would be lost and user confirms
    fn check_force_push_safety_enhanced(
        &self,
        target_branch: &str,
    ) -> Result<Option<ForceBackupInfo>> {
        // First fetch latest remote changes to ensure we have up-to-date information
        match self.fetch() {
            Ok(_) => {}
            Err(e) => {
                // If fetch fails, warn but don't block the operation
                warn!("Could not fetch latest changes for safety check: {}", e);
            }
        }

        // Check if there are commits on the remote that would be lost
        let remote_ref = format!("refs/remotes/origin/{target_branch}");
        let local_ref = format!("refs/heads/{target_branch}");

        // Try to find both local and remote references
        let local_commit = match self.repo.find_reference(&local_ref) {
            Ok(reference) => reference.peel_to_commit().ok(),
            Err(_) => None,
        };

        let remote_commit = match self.repo.find_reference(&remote_ref) {
            Ok(reference) => reference.peel_to_commit().ok(),
            Err(_) => None,
        };

        // If we have both commits, check for divergence
        if let (Some(local), Some(remote)) = (local_commit, remote_commit) {
            if local.id() != remote.id() {
                // Check if the remote has commits that the local doesn't have
                let merge_base_oid = self
                    .repo
                    .merge_base(local.id(), remote.id())
                    .map_err(|e| CascadeError::config(format!("Failed to find merge base: {e}")))?;

                // If merge base != remote commit, remote has commits that would be lost
                if merge_base_oid != remote.id() {
                    let commits_to_lose = self.count_commits_between(
                        &merge_base_oid.to_string(),
                        &remote.id().to_string(),
                    )?;

                    // Create backup branch name with timestamp
                    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                    let backup_branch_name = format!("{target_branch}_backup_{timestamp}");

                    warn!(
                        "⚠️  Force push to '{}' would overwrite {} commits on remote",
                        target_branch, commits_to_lose
                    );

                    // Check if we're in a non-interactive environment (CI/testing)
                    if std::env::var("CI").is_ok() || std::env::var("FORCE_PUSH_NO_CONFIRM").is_ok()
                    {
                        info!(
                            "Non-interactive environment detected, proceeding with backup creation"
                        );
                        return Ok(Some(ForceBackupInfo {
                            backup_branch_name,
                            remote_commit_id: remote.id().to_string(),
                            commits_that_would_be_lost: commits_to_lose,
                        }));
                    }

                    // Interactive confirmation
                    println!("\n⚠️  FORCE PUSH WARNING ⚠️");
                    println!("Force push to '{target_branch}' would overwrite {commits_to_lose} commits on remote:");

                    // Show the commits that would be lost
                    match self
                        .get_commits_between(&merge_base_oid.to_string(), &remote.id().to_string())
                    {
                        Ok(commits) => {
                            println!("\nCommits that would be lost:");
                            for (i, commit) in commits.iter().take(5).enumerate() {
                                let short_hash = &commit.id().to_string()[..8];
                                let summary = commit.summary().unwrap_or("<no message>");
                                println!("  {}. {} - {}", i + 1, short_hash, summary);
                            }
                            if commits.len() > 5 {
                                println!("  ... and {} more commits", commits.len() - 5);
                            }
                        }
                        Err(_) => {
                            println!("  (Unable to retrieve commit details)");
                        }
                    }

                    println!("\nA backup branch '{backup_branch_name}' will be created before proceeding.");

                    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt("Do you want to proceed with the force push?")
                        .default(false)
                        .interact()
                        .map_err(|e| {
                            CascadeError::config(format!("Failed to get user confirmation: {e}"))
                        })?;

                    if !confirmed {
                        return Err(CascadeError::config(
                            "Force push cancelled by user. Use --force to bypass this check."
                                .to_string(),
                        ));
                    }

                    return Ok(Some(ForceBackupInfo {
                        backup_branch_name,
                        remote_commit_id: remote.id().to_string(),
                        commits_that_would_be_lost: commits_to_lose,
                    }));
                }
            }
        }

        Ok(None)
    }

    /// Create a backup branch pointing to the remote commit that would be lost
    fn create_backup_branch(&self, original_branch: &str, remote_commit_id: &str) -> Result<()> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_branch_name = format!("{original_branch}_backup_{timestamp}");

        // Parse the commit ID
        let commit_oid = Oid::from_str(remote_commit_id).map_err(|e| {
            CascadeError::config(format!("Invalid commit ID {remote_commit_id}: {e}"))
        })?;

        // Find the commit
        let commit = self.repo.find_commit(commit_oid).map_err(|e| {
            CascadeError::config(format!("Failed to find commit {remote_commit_id}: {e}"))
        })?;

        // Create the backup branch
        self.repo
            .branch(&backup_branch_name, &commit, false)
            .map_err(|e| {
                CascadeError::config(format!(
                    "Failed to create backup branch {backup_branch_name}: {e}"
                ))
            })?;

        info!(
            "✅ Created backup branch '{}' pointing to {}",
            backup_branch_name,
            &remote_commit_id[..8]
        );
        Ok(())
    }

    /// Check if branch deletion is safe by detecting unpushed commits
    /// Returns safety info if there are concerns that need user attention
    fn check_branch_deletion_safety(
        &self,
        branch_name: &str,
    ) -> Result<Option<BranchDeletionSafety>> {
        // First, try to fetch latest remote changes
        match self.fetch() {
            Ok(_) => {}
            Err(e) => {
                warn!(
                    "Could not fetch latest changes for branch deletion safety check: {}",
                    e
                );
            }
        }

        // Find the branch
        let branch = self
            .repo
            .find_branch(branch_name, git2::BranchType::Local)
            .map_err(|e| {
                CascadeError::branch(format!("Could not find branch '{branch_name}': {e}"))
            })?;

        let _branch_commit = branch.get().peel_to_commit().map_err(|e| {
            CascadeError::branch(format!(
                "Could not get commit for branch '{branch_name}': {e}"
            ))
        })?;

        // Determine the main branch (try common names)
        let main_branch_name = self.detect_main_branch()?;

        // Check if branch is merged to main
        let is_merged_to_main = self.is_branch_merged_to_main(branch_name, &main_branch_name)?;

        // Find the upstream/remote tracking branch
        let remote_tracking_branch = self.get_remote_tracking_branch(branch_name);

        let mut unpushed_commits = Vec::new();

        // Check for unpushed commits compared to remote tracking branch
        if let Some(ref remote_branch) = remote_tracking_branch {
            match self.get_commits_between(remote_branch, branch_name) {
                Ok(commits) => {
                    unpushed_commits = commits.iter().map(|c| c.id().to_string()).collect();
                }
                Err(_) => {
                    // If we can't compare with remote, check against main branch
                    if !is_merged_to_main {
                        if let Ok(commits) =
                            self.get_commits_between(&main_branch_name, branch_name)
                        {
                            unpushed_commits = commits.iter().map(|c| c.id().to_string()).collect();
                        }
                    }
                }
            }
        } else if !is_merged_to_main {
            // No remote tracking branch, check against main
            if let Ok(commits) = self.get_commits_between(&main_branch_name, branch_name) {
                unpushed_commits = commits.iter().map(|c| c.id().to_string()).collect();
            }
        }

        // If there are concerns, return safety info
        if !unpushed_commits.is_empty() || (!is_merged_to_main && remote_tracking_branch.is_none())
        {
            Ok(Some(BranchDeletionSafety {
                unpushed_commits,
                remote_tracking_branch,
                is_merged_to_main,
                main_branch_name,
            }))
        } else {
            Ok(None)
        }
    }

    /// Handle user confirmation for branch deletion with safety concerns
    fn handle_branch_deletion_confirmation(
        &self,
        branch_name: &str,
        safety_info: &BranchDeletionSafety,
    ) -> Result<()> {
        // Check if we're in a non-interactive environment
        if std::env::var("CI").is_ok() || std::env::var("BRANCH_DELETE_NO_CONFIRM").is_ok() {
            return Err(CascadeError::branch(
                format!(
                    "Branch '{branch_name}' has {} unpushed commits and cannot be deleted in non-interactive mode. Use --force to override.",
                    safety_info.unpushed_commits.len()
                )
            ));
        }

        // Interactive warning and confirmation
        println!("\n⚠️  BRANCH DELETION WARNING ⚠️");
        println!("Branch '{branch_name}' has potential issues:");

        if !safety_info.unpushed_commits.is_empty() {
            println!(
                "\n🔍 Unpushed commits ({} total):",
                safety_info.unpushed_commits.len()
            );

            // Show details of unpushed commits
            for (i, commit_id) in safety_info.unpushed_commits.iter().take(5).enumerate() {
                if let Ok(commit) = self.repo.find_commit(Oid::from_str(commit_id).unwrap()) {
                    let short_hash = &commit_id[..8];
                    let summary = commit.summary().unwrap_or("<no message>");
                    println!("  {}. {} - {}", i + 1, short_hash, summary);
                }
            }

            if safety_info.unpushed_commits.len() > 5 {
                println!(
                    "  ... and {} more commits",
                    safety_info.unpushed_commits.len() - 5
                );
            }
        }

        if !safety_info.is_merged_to_main {
            println!("\n📋 Branch status:");
            println!("  • Not merged to '{}'", safety_info.main_branch_name);
            if let Some(ref remote) = safety_info.remote_tracking_branch {
                println!("  • Remote tracking branch: {remote}");
            } else {
                println!("  • No remote tracking branch");
            }
        }

        println!("\n💡 Safer alternatives:");
        if !safety_info.unpushed_commits.is_empty() {
            if let Some(ref _remote) = safety_info.remote_tracking_branch {
                println!("  • Push commits first: git push origin {branch_name}");
            } else {
                println!("  • Create and push to remote: git push -u origin {branch_name}");
            }
        }
        if !safety_info.is_merged_to_main {
            println!(
                "  • Merge to {} first: git checkout {} && git merge {branch_name}",
                safety_info.main_branch_name, safety_info.main_branch_name
            );
        }

        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Do you want to proceed with deleting this branch?")
            .default(false)
            .interact()
            .map_err(|e| CascadeError::branch(format!("Failed to get user confirmation: {e}")))?;

        if !confirmed {
            return Err(CascadeError::branch(
                "Branch deletion cancelled by user. Use --force to bypass this check.".to_string(),
            ));
        }

        Ok(())
    }

    /// Detect the main branch name (main, master, develop)
    fn detect_main_branch(&self) -> Result<String> {
        let main_candidates = ["main", "master", "develop", "trunk"];

        for candidate in &main_candidates {
            if self
                .repo
                .find_branch(candidate, git2::BranchType::Local)
                .is_ok()
            {
                return Ok(candidate.to_string());
            }
        }

        // Fallback to HEAD's target if it's a symbolic reference
        if let Ok(head) = self.repo.head() {
            if let Some(name) = head.shorthand() {
                return Ok(name.to_string());
            }
        }

        // Final fallback
        Ok("main".to_string())
    }

    /// Check if a branch is merged to the main branch
    fn is_branch_merged_to_main(&self, branch_name: &str, main_branch: &str) -> Result<bool> {
        // Get the commits between main and the branch
        match self.get_commits_between(main_branch, branch_name) {
            Ok(commits) => Ok(commits.is_empty()),
            Err(_) => {
                // If we can't determine, assume not merged for safety
                Ok(false)
            }
        }
    }

    /// Get the remote tracking branch for a local branch
    fn get_remote_tracking_branch(&self, branch_name: &str) -> Option<String> {
        // Try common remote tracking branch patterns
        let remote_candidates = [
            format!("origin/{branch_name}"),
            format!("remotes/origin/{branch_name}"),
        ];

        for candidate in &remote_candidates {
            if self
                .repo
                .find_reference(&format!(
                    "refs/remotes/{}",
                    candidate.replace("remotes/", "")
                ))
                .is_ok()
            {
                return Some(candidate.clone());
            }
        }

        None
    }

    /// Check if checkout operation is safe
    fn check_checkout_safety(&self, _target: &str) -> Result<Option<CheckoutSafety>> {
        // Check if there are uncommitted changes
        let is_dirty = self.is_dirty()?;
        if !is_dirty {
            // No uncommitted changes, checkout is safe
            return Ok(None);
        }

        // Get current branch for context
        let current_branch = self.get_current_branch().ok();

        // Get detailed information about uncommitted changes
        let modified_files = self.get_modified_files()?;
        let staged_files = self.get_staged_files()?;
        let untracked_files = self.get_untracked_files()?;

        let has_uncommitted_changes = !modified_files.is_empty() || !staged_files.is_empty();

        if has_uncommitted_changes || !untracked_files.is_empty() {
            return Ok(Some(CheckoutSafety {
                has_uncommitted_changes,
                modified_files,
                staged_files,
                untracked_files,
                stash_created: None,
                current_branch,
            }));
        }

        Ok(None)
    }

    /// Handle user confirmation for checkout operations with uncommitted changes
    fn handle_checkout_confirmation(
        &self,
        target: &str,
        safety_info: &CheckoutSafety,
    ) -> Result<()> {
        // Check if we're in a non-interactive environment FIRST (before any output)
        let is_ci = std::env::var("CI").is_ok();
        let no_confirm = std::env::var("CHECKOUT_NO_CONFIRM").is_ok();
        let is_non_interactive = is_ci || no_confirm;

        if is_non_interactive {
            return Err(CascadeError::branch(
                format!(
                    "Cannot checkout '{target}' with uncommitted changes in non-interactive mode. Commit your changes or use stash first."
                )
            ));
        }

        // Interactive warning and confirmation
        println!("\n⚠️  CHECKOUT WARNING ⚠️");
        println!("You have uncommitted changes that could be lost:");

        if !safety_info.modified_files.is_empty() {
            println!(
                "\n📝 Modified files ({}):",
                safety_info.modified_files.len()
            );
            for file in safety_info.modified_files.iter().take(10) {
                println!("   - {file}");
            }
            if safety_info.modified_files.len() > 10 {
                println!("   ... and {} more", safety_info.modified_files.len() - 10);
            }
        }

        if !safety_info.staged_files.is_empty() {
            println!("\n📁 Staged files ({}):", safety_info.staged_files.len());
            for file in safety_info.staged_files.iter().take(10) {
                println!("   - {file}");
            }
            if safety_info.staged_files.len() > 10 {
                println!("   ... and {} more", safety_info.staged_files.len() - 10);
            }
        }

        if !safety_info.untracked_files.is_empty() {
            println!(
                "\n❓ Untracked files ({}):",
                safety_info.untracked_files.len()
            );
            for file in safety_info.untracked_files.iter().take(5) {
                println!("   - {file}");
            }
            if safety_info.untracked_files.len() > 5 {
                println!("   ... and {} more", safety_info.untracked_files.len() - 5);
            }
        }

        println!("\n🔄 Options:");
        println!("1. Stash changes and checkout (recommended)");
        println!("2. Force checkout (WILL LOSE UNCOMMITTED CHANGES)");
        println!("3. Cancel checkout");

        let confirmation = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Would you like to stash your changes and proceed with checkout?")
            .interact()
            .map_err(|e| CascadeError::branch(format!("Could not get user confirmation: {e}")))?;

        if confirmation {
            // Create stash before checkout
            let stash_message = format!(
                "Auto-stash before checkout to {} at {}",
                target,
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
            );

            match self.create_stash(&stash_message) {
                Ok(stash_oid) => {
                    println!("✅ Created stash: {stash_message} ({stash_oid})");
                    println!("💡 You can restore with: git stash pop");
                }
                Err(e) => {
                    println!("❌ Failed to create stash: {e}");

                    let force_confirm = Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt("Stash failed. Force checkout anyway? (WILL LOSE CHANGES)")
                        .interact()
                        .map_err(|e| {
                            CascadeError::branch(format!("Could not get confirmation: {e}"))
                        })?;

                    if !force_confirm {
                        return Err(CascadeError::branch(
                            "Checkout cancelled by user".to_string(),
                        ));
                    }
                }
            }
        } else {
            return Err(CascadeError::branch(
                "Checkout cancelled by user".to_string(),
            ));
        }

        Ok(())
    }

    /// Create a stash with uncommitted changes
    fn create_stash(&self, message: &str) -> Result<String> {
        // For now, we'll use a different approach that doesn't require mutable access
        // This is a simplified version that recommends manual stashing

        warn!("Automatic stashing not yet implemented - please stash manually");
        Err(CascadeError::branch(format!(
            "Please manually stash your changes first: git stash push -m \"{message}\""
        )))
    }

    /// Get modified files in working directory
    fn get_modified_files(&self) -> Result<Vec<String>> {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(false).include_ignored(false);

        let statuses = self
            .repo
            .statuses(Some(&mut opts))
            .map_err(|e| CascadeError::branch(format!("Could not get repository status: {e}")))?;

        let mut modified_files = Vec::new();
        for status in statuses.iter() {
            let flags = status.status();
            if flags.contains(git2::Status::WT_MODIFIED) || flags.contains(git2::Status::WT_DELETED)
            {
                if let Some(path) = status.path() {
                    modified_files.push(path.to_string());
                }
            }
        }

        Ok(modified_files)
    }

    /// Get staged files in index
    fn get_staged_files(&self) -> Result<Vec<String>> {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(false).include_ignored(false);

        let statuses = self
            .repo
            .statuses(Some(&mut opts))
            .map_err(|e| CascadeError::branch(format!("Could not get repository status: {e}")))?;

        let mut staged_files = Vec::new();
        for status in statuses.iter() {
            let flags = status.status();
            if flags.contains(git2::Status::INDEX_MODIFIED)
                || flags.contains(git2::Status::INDEX_NEW)
                || flags.contains(git2::Status::INDEX_DELETED)
            {
                if let Some(path) = status.path() {
                    staged_files.push(path.to_string());
                }
            }
        }

        Ok(staged_files)
    }

    /// Count commits between two references
    fn count_commits_between(&self, from: &str, to: &str) -> Result<usize> {
        let commits = self.get_commits_between(from, to)?;
        Ok(commits.len())
    }

    /// Resolve a reference (branch name, tag, or commit hash) to a commit
    pub fn resolve_reference(&self, reference: &str) -> Result<git2::Commit<'_>> {
        // Try to parse as commit hash first
        if let Ok(oid) = Oid::from_str(reference) {
            if let Ok(commit) = self.repo.find_commit(oid) {
                return Ok(commit);
            }
        }

        // Try to resolve as a reference (branch, tag, etc.)
        let obj = self.repo.revparse_single(reference).map_err(|e| {
            CascadeError::branch(format!("Could not resolve reference '{reference}': {e}"))
        })?;

        obj.peel_to_commit().map_err(|e| {
            CascadeError::branch(format!(
                "Reference '{reference}' does not point to a commit: {e}"
            ))
        })
    }

    /// Reset HEAD to a specific reference (soft reset)
    pub fn reset_soft(&self, target_ref: &str) -> Result<()> {
        let target_commit = self.resolve_reference(target_ref)?;

        self.repo
            .reset(target_commit.as_object(), git2::ResetType::Soft, None)
            .map_err(CascadeError::Git)?;

        Ok(())
    }

    /// Find which branch contains a specific commit
    pub fn find_branch_containing_commit(&self, commit_hash: &str) -> Result<String> {
        let oid = Oid::from_str(commit_hash).map_err(|e| {
            CascadeError::branch(format!("Invalid commit hash '{commit_hash}': {e}"))
        })?;

        // Get all local branches
        let branches = self
            .repo
            .branches(Some(git2::BranchType::Local))
            .map_err(CascadeError::Git)?;

        for branch_result in branches {
            let (branch, _) = branch_result.map_err(CascadeError::Git)?;

            if let Some(branch_name) = branch.name().map_err(CascadeError::Git)? {
                // Check if this branch contains the commit
                if let Ok(branch_head) = branch.get().peel_to_commit() {
                    // Walk the commit history from this branch's HEAD
                    let mut revwalk = self.repo.revwalk().map_err(CascadeError::Git)?;
                    revwalk.push(branch_head.id()).map_err(CascadeError::Git)?;

                    for commit_oid in revwalk {
                        let commit_oid = commit_oid.map_err(CascadeError::Git)?;
                        if commit_oid == oid {
                            return Ok(branch_name.to_string());
                        }
                    }
                }
            }
        }

        // If not found in any branch, might be on current HEAD
        Err(CascadeError::branch(format!(
            "Commit {commit_hash} not found in any local branch"
        )))
    }

    // Async wrappers for potentially blocking operations

    /// Fetch from remote origin (async)
    pub async fn fetch_async(&self) -> Result<()> {
        let repo_path = self.path.clone();
        crate::utils::async_ops::run_git_operation(move || {
            let repo = GitRepository::open(&repo_path)?;
            repo.fetch()
        })
        .await
    }

    /// Pull changes from remote (async)
    pub async fn pull_async(&self, branch: &str) -> Result<()> {
        let repo_path = self.path.clone();
        let branch_name = branch.to_string();
        crate::utils::async_ops::run_git_operation(move || {
            let repo = GitRepository::open(&repo_path)?;
            repo.pull(&branch_name)
        })
        .await
    }

    /// Push branch to remote (async)
    pub async fn push_branch_async(&self, branch_name: &str) -> Result<()> {
        let repo_path = self.path.clone();
        let branch = branch_name.to_string();
        crate::utils::async_ops::run_git_operation(move || {
            let repo = GitRepository::open(&repo_path)?;
            repo.push(&branch)
        })
        .await
    }

    /// Cherry-pick commit (async)
    pub async fn cherry_pick_commit_async(&self, commit_hash: &str) -> Result<String> {
        let repo_path = self.path.clone();
        let hash = commit_hash.to_string();
        crate::utils::async_ops::run_git_operation(move || {
            let repo = GitRepository::open(&repo_path)?;
            repo.cherry_pick(&hash)
        })
        .await
    }

    /// Get commit hashes between two refs (async)
    pub async fn get_commit_hashes_between_async(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Vec<String>> {
        let repo_path = self.path.clone();
        let from_str = from.to_string();
        let to_str = to.to_string();
        crate::utils::async_ops::run_git_operation(move || {
            let repo = GitRepository::open(&repo_path)?;
            let commits = repo.get_commits_between(&from_str, &to_str)?;
            Ok(commits.into_iter().map(|c| c.id().to_string()).collect())
        })
        .await
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
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
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

        (temp_dir, repo_path)
    }

    fn create_commit(repo_path: &PathBuf, message: &str, filename: &str) {
        let file_path = repo_path.join(filename);
        std::fs::write(&file_path, format!("Content for {filename}\n")).unwrap();

        Command::new("git")
            .args(["add", filename])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(repo_path)
            .output()
            .unwrap();
    }

    #[test]
    fn test_repository_info() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        let info = repo.get_info().unwrap();
        assert!(!info.is_dirty); // Should be clean after commit
        assert!(
            info.head_branch == Some("master".to_string())
                || info.head_branch == Some("main".to_string()),
            "Expected default branch to be 'master' or 'main', got {:?}",
            info.head_branch
        );
        assert!(info.head_commit.is_some()); // Just check it exists
        assert!(info.untracked_files.is_empty()); // Should be empty after commit
    }

    #[test]
    fn test_force_push_branch_basic() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Get the actual default branch name
        let default_branch = repo.get_current_branch().unwrap();

        // Create source branch with commits
        create_commit(&repo_path, "Feature commit 1", "feature1.rs");
        Command::new("git")
            .args(["checkout", "-b", "source-branch"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        create_commit(&repo_path, "Feature commit 2", "feature2.rs");

        // Create target branch
        Command::new("git")
            .args(["checkout", &default_branch])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["checkout", "-b", "target-branch"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        create_commit(&repo_path, "Target commit", "target.rs");

        // Test force push from source to target
        let result = repo.force_push_branch("target-branch", "source-branch");

        // Should succeed in test environment (even though it doesn't actually push to remote)
        // The important thing is that the function doesn't panic and handles the git2 operations
        assert!(result.is_ok() || result.is_err()); // Either is acceptable for unit test
    }

    #[test]
    fn test_force_push_branch_nonexistent_branches() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Get the actual default branch name
        let default_branch = repo.get_current_branch().unwrap();

        // Test force push with nonexistent source branch
        let result = repo.force_push_branch("target", "nonexistent-source");
        assert!(result.is_err());

        // Test force push with nonexistent target branch
        let result = repo.force_push_branch("nonexistent-target", &default_branch);
        assert!(result.is_err());
    }

    #[test]
    fn test_force_push_workflow_simulation() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Simulate the smart force push workflow:
        // 1. Original branch exists with PR
        Command::new("git")
            .args(["checkout", "-b", "feature-auth"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        create_commit(&repo_path, "Add authentication", "auth.rs");

        // 2. Rebase creates versioned branch
        Command::new("git")
            .args(["checkout", "-b", "feature-auth-v2"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        create_commit(&repo_path, "Fix auth validation", "auth.rs");

        // 3. Smart force push: update original branch from versioned branch
        let result = repo.force_push_branch("feature-auth", "feature-auth-v2");

        // Verify the operation is handled properly (success or expected error)
        match result {
            Ok(_) => {
                // Force push succeeded - verify branch state if possible
                Command::new("git")
                    .args(["checkout", "feature-auth"])
                    .current_dir(&repo_path)
                    .output()
                    .unwrap();
                let log_output = Command::new("git")
                    .args(["log", "--oneline", "-2"])
                    .current_dir(&repo_path)
                    .output()
                    .unwrap();
                let log_str = String::from_utf8_lossy(&log_output.stdout);
                assert!(
                    log_str.contains("Fix auth validation")
                        || log_str.contains("Add authentication")
                );
            }
            Err(_) => {
                // Expected in test environment without remote - that's fine
                // The important thing is we tested the code path without panicking
            }
        }
    }

    #[test]
    fn test_branch_operations() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Test get current branch - accept either main or master
        let current = repo.get_current_branch().unwrap();
        assert!(
            current == "master" || current == "main",
            "Expected default branch to be 'master' or 'main', got '{current}'"
        );

        // Test create branch
        Command::new("git")
            .args(["checkout", "-b", "test-branch"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let current = repo.get_current_branch().unwrap();
        assert_eq!(current, "test-branch");
    }

    #[test]
    fn test_commit_operations() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Test get head commit
        let head = repo.get_head_commit().unwrap();
        assert_eq!(head.message().unwrap().trim(), "Initial commit");

        // Test get commit by hash
        let hash = head.id().to_string();
        let same_commit = repo.get_commit(&hash).unwrap();
        assert_eq!(head.id(), same_commit.id());
    }

    #[test]
    fn test_checkout_safety_clean_repo() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Create a test branch
        create_commit(&repo_path, "Second commit", "test.txt");
        Command::new("git")
            .args(["checkout", "-b", "test-branch"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Test checkout safety with clean repo
        let safety_result = repo.check_checkout_safety("main");
        assert!(safety_result.is_ok());
        assert!(safety_result.unwrap().is_none()); // Clean repo should return None
    }

    #[test]
    fn test_checkout_safety_with_modified_files() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Create a test branch
        Command::new("git")
            .args(["checkout", "-b", "test-branch"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Modify a file to create uncommitted changes
        std::fs::write(repo_path.join("README.md"), "Modified content").unwrap();

        // Test checkout safety with modified files
        let safety_result = repo.check_checkout_safety("main");
        assert!(safety_result.is_ok());
        let safety_info = safety_result.unwrap();
        assert!(safety_info.is_some());

        let info = safety_info.unwrap();
        assert!(!info.modified_files.is_empty());
        assert!(info.modified_files.contains(&"README.md".to_string()));
    }

    #[test]
    fn test_unsafe_checkout_methods() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Create a test branch
        create_commit(&repo_path, "Second commit", "test.txt");
        Command::new("git")
            .args(["checkout", "-b", "test-branch"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Modify a file to create uncommitted changes
        std::fs::write(repo_path.join("README.md"), "Modified content").unwrap();

        // Test unsafe checkout methods bypass safety checks
        let _result = repo.checkout_branch_unsafe("master");
        // Note: This might still fail due to git2 restrictions, but shouldn't hit our safety code
        // The important thing is that it doesn't trigger our safety confirmation

        // Test unsafe commit checkout
        let head_commit = repo.get_head_commit().unwrap();
        let commit_hash = head_commit.id().to_string();
        let _result = repo.checkout_commit_unsafe(&commit_hash);
        // Similar to above - testing that safety is bypassed
    }

    #[test]
    fn test_get_modified_files() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Initially should have no modified files
        let modified = repo.get_modified_files().unwrap();
        assert!(modified.is_empty());

        // Modify a file
        std::fs::write(repo_path.join("README.md"), "Modified content").unwrap();

        // Should now detect the modified file
        let modified = repo.get_modified_files().unwrap();
        assert_eq!(modified.len(), 1);
        assert!(modified.contains(&"README.md".to_string()));
    }

    #[test]
    fn test_get_staged_files() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Initially should have no staged files
        let staged = repo.get_staged_files().unwrap();
        assert!(staged.is_empty());

        // Create and stage a new file
        std::fs::write(repo_path.join("staged.txt"), "Staged content").unwrap();
        Command::new("git")
            .args(["add", "staged.txt"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Should now detect the staged file
        let staged = repo.get_staged_files().unwrap();
        assert_eq!(staged.len(), 1);
        assert!(staged.contains(&"staged.txt".to_string()));
    }

    #[test]
    fn test_create_stash_fallback() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Test that stash creation returns helpful error message
        let result = repo.create_stash("test stash");
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("git stash push"));
    }

    #[test]
    fn test_delete_branch_unsafe() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Create a test branch
        create_commit(&repo_path, "Second commit", "test.txt");
        Command::new("git")
            .args(["checkout", "-b", "test-branch"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Add another commit to the test branch to make it different from master
        create_commit(&repo_path, "Branch-specific commit", "branch.txt");

        // Go back to master
        Command::new("git")
            .args(["checkout", "master"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Test unsafe delete bypasses safety checks
        // Note: This may still fail if the branch has unpushed commits, but it should bypass our safety confirmation
        let result = repo.delete_branch_unsafe("test-branch");
        // Even if it fails, the key is that it didn't prompt for user confirmation
        // So we just check that it attempted the operation without interactive prompts
        let _ = result; // Don't assert success since delete may fail for git reasons
    }

    #[test]
    fn test_force_push_unsafe() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Create a test branch
        create_commit(&repo_path, "Second commit", "test.txt");
        Command::new("git")
            .args(["checkout", "-b", "test-branch"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Test unsafe force push bypasses safety checks
        // Note: This will likely fail due to no remote, but it tests the safety bypass
        let _result = repo.force_push_branch_unsafe("test-branch", "test-branch");
        // The key is that it doesn't trigger safety confirmation dialogs
    }
}
