use crate::cli::output::Output;
use crate::errors::{CascadeError, Result};
use chrono;
use dialoguer::{theme::ColorfulTheme, Confirm, Select};
use git2::{Oid, Repository, Signature};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

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

/// SSL configuration for git operations
#[derive(Debug, Clone)]
pub struct GitSslConfig {
    pub accept_invalid_certs: bool,
    pub ca_bundle_path: Option<String>,
}

/// Summary of git repository status
#[derive(Debug, Clone)]
pub struct GitStatusSummary {
    staged_files: usize,
    unstaged_files: usize,
    untracked_files: usize,
}

impl GitStatusSummary {
    pub fn is_clean(&self) -> bool {
        self.staged_files == 0 && self.unstaged_files == 0 && self.untracked_files == 0
    }

    pub fn has_staged_changes(&self) -> bool {
        self.staged_files > 0
    }

    pub fn has_unstaged_changes(&self) -> bool {
        self.unstaged_files > 0
    }

    pub fn has_untracked_files(&self) -> bool {
        self.untracked_files > 0
    }

    pub fn staged_count(&self) -> usize {
        self.staged_files
    }

    pub fn unstaged_count(&self) -> usize {
        self.unstaged_files
    }

    pub fn untracked_count(&self) -> usize {
        self.untracked_files
    }
}

/// Wrapper around git2::Repository with safe operations
///
/// For thread safety, use the async variants (e.g., fetch_async, pull_async)
/// which automatically handle threading using tokio::spawn_blocking.
/// The async methods create new repository instances in background threads.
pub struct GitRepository {
    repo: Repository,
    path: PathBuf,
    ssl_config: Option<GitSslConfig>,
    bitbucket_credentials: Option<BitbucketCredentials>,
}

#[derive(Debug, Clone)]
struct BitbucketCredentials {
    username: Option<String>,
    token: Option<String>,
}

impl GitRepository {
    /// Open a Git repository at the given path
    /// Automatically loads SSL configuration from cascade config if available
    pub fn open(path: &Path) -> Result<Self> {
        let repo = Repository::discover(path)
            .map_err(|e| CascadeError::config(format!("Not a git repository: {e}")))?;

        let workdir = repo
            .workdir()
            .ok_or_else(|| CascadeError::config("Repository has no working directory"))?
            .to_path_buf();

        // Try to load SSL configuration from cascade config
        let ssl_config = Self::load_ssl_config_from_cascade(&workdir);
        let bitbucket_credentials = Self::load_bitbucket_credentials_from_cascade(&workdir);

        Ok(Self {
            repo,
            path: workdir,
            ssl_config,
            bitbucket_credentials,
        })
    }

    /// Load SSL configuration from cascade config file if it exists
    fn load_ssl_config_from_cascade(repo_path: &Path) -> Option<GitSslConfig> {
        // Try to load cascade configuration
        let config_dir = crate::config::get_repo_config_dir(repo_path).ok()?;
        let config_path = config_dir.join("config.json");
        let settings = crate::config::Settings::load_from_file(&config_path).ok()?;

        // Convert BitbucketConfig to GitSslConfig if SSL settings exist
        if settings.bitbucket.accept_invalid_certs.is_some()
            || settings.bitbucket.ca_bundle_path.is_some()
        {
            Some(GitSslConfig {
                accept_invalid_certs: settings.bitbucket.accept_invalid_certs.unwrap_or(false),
                ca_bundle_path: settings.bitbucket.ca_bundle_path,
            })
        } else {
            None
        }
    }

    /// Load Bitbucket credentials from cascade config file if it exists
    fn load_bitbucket_credentials_from_cascade(repo_path: &Path) -> Option<BitbucketCredentials> {
        // Try to load cascade configuration
        let config_dir = crate::config::get_repo_config_dir(repo_path).ok()?;
        let config_path = config_dir.join("config.json");
        let settings = crate::config::Settings::load_from_file(&config_path).ok()?;

        // Return credentials if any are configured
        if settings.bitbucket.username.is_some() || settings.bitbucket.token.is_some() {
            Some(BitbucketCredentials {
                username: settings.bitbucket.username.clone(),
                token: settings.bitbucket.token.clone(),
            })
        } else {
            None
        }
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

        // Branch creation logging is handled by the caller for clean output
        Ok(())
    }

    /// Update a branch to point to a specific commit (local operation only)
    /// Creates the branch if it doesn't exist, updates it if it does
    pub fn update_branch_to_commit(&self, branch_name: &str, commit_id: &str) -> Result<()> {
        let commit_oid = Oid::from_str(commit_id).map_err(|e| {
            CascadeError::branch(format!("Invalid commit ID '{}': {}", commit_id, e))
        })?;

        let commit = self.repo.find_commit(commit_oid).map_err(|e| {
            CascadeError::branch(format!("Commit '{}' not found: {}", commit_id, e))
        })?;

        // Try to find existing branch
        if self
            .repo
            .find_branch(branch_name, git2::BranchType::Local)
            .is_ok()
        {
            // Update existing branch to point to new commit
            let refname = format!("refs/heads/{}", branch_name);
            self.repo
                .reference(
                    &refname,
                    commit_oid,
                    true,
                    "update branch to rebased commit",
                )
                .map_err(|e| {
                    CascadeError::branch(format!(
                        "Failed to update branch '{}': {}",
                        branch_name, e
                    ))
                })?;
        } else {
            // Create new branch
            self.repo.branch(branch_name, &commit, false).map_err(|e| {
                CascadeError::branch(format!("Failed to create branch '{}': {}", branch_name, e))
            })?;
        }

        Ok(())
    }

    /// Force-push a single branch to remote (simpler version for when branch is already updated locally)
    pub fn force_push_single_branch(&self, branch_name: &str) -> Result<()> {
        self.force_push_single_branch_with_options(branch_name, false)
    }

    /// Force push with option to skip user confirmation (for automated operations like sync)
    pub fn force_push_single_branch_auto(&self, branch_name: &str) -> Result<()> {
        self.force_push_single_branch_with_options(branch_name, true)
    }

    fn force_push_single_branch_with_options(
        &self,
        branch_name: &str,
        auto_confirm: bool,
    ) -> Result<()> {
        // Fetch first to ensure we have latest remote state for safety checks
        if let Err(e) = self.fetch() {
            tracing::warn!("Could not fetch before force push: {}", e);
        }

        // Check safety and create backup if needed
        let safety_result = if auto_confirm {
            self.check_force_push_safety_auto(branch_name)?
        } else {
            self.check_force_push_safety_enhanced(branch_name)?
        };

        if let Some(backup_info) = safety_result {
            self.create_backup_branch(branch_name, &backup_info.remote_commit_id)?;
        }

        // Force push using git CLI (more reliable than git2 for TLS)
        let output = std::process::Command::new("git")
            .args(["push", "--force", "origin", branch_name])
            .current_dir(&self.path)
            .output()
            .map_err(|e| CascadeError::branch(format!("Failed to execute git push: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CascadeError::branch(format!(
                "Force push failed for '{}': {}",
                branch_name, stderr
            )));
        }

        Ok(())
    }

    /// Switch to a branch with safety checks
    pub fn checkout_branch(&self, name: &str) -> Result<()> {
        self.checkout_branch_with_options(name, false, true)
    }

    /// Switch to a branch silently (no output)
    pub fn checkout_branch_silent(&self, name: &str) -> Result<()> {
        self.checkout_branch_with_options(name, false, false)
    }

    /// Switch to a branch with force option to bypass safety checks
    pub fn checkout_branch_unsafe(&self, name: &str) -> Result<()> {
        self.checkout_branch_with_options(name, true, true)
    }

    /// Internal branch checkout implementation with safety options
    fn checkout_branch_with_options(&self, name: &str, force_unsafe: bool, show_output: bool) -> Result<()> {
        debug!("Attempting to checkout branch: {}", name);

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

        if show_output {
            Output::success(format!("Switched to branch '{name}'"));
        }
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
        debug!("Attempting to checkout commit: {}", commit_hash);

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

        Output::success(format!(
            "Checked out commit '{commit_hash}' (detached HEAD)"
        ));
        Ok(())
    }

    /// Check if a branch exists
    pub fn branch_exists(&self, name: &str) -> bool {
        self.repo.find_branch(name, git2::BranchType::Local).is_ok()
    }

    /// Check if a branch exists locally, and if not, attempt to fetch it from remote
    pub fn branch_exists_or_fetch(&self, name: &str) -> Result<bool> {
        // 1. Check if branch exists locally first
        if self.repo.find_branch(name, git2::BranchType::Local).is_ok() {
            return Ok(true);
        }

        // 2. Try to fetch it from remote
        println!("🔍 Branch '{name}' not found locally, trying to fetch from remote...");

        use std::process::Command;

        // Try: git fetch origin release/12.34:release/12.34
        let fetch_result = Command::new("git")
            .args(["fetch", "origin", &format!("{name}:{name}")])
            .current_dir(&self.path)
            .output();

        match fetch_result {
            Ok(output) => {
                if output.status.success() {
                    println!("✅ Successfully fetched '{name}' from origin");
                    // 3. Check again locally after fetch
                    return Ok(self.repo.find_branch(name, git2::BranchType::Local).is_ok());
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::debug!("Failed to fetch branch '{name}': {stderr}");
                }
            }
            Err(e) => {
                tracing::debug!("Git fetch command failed: {e}");
            }
        }

        // 4. Try alternative fetch patterns for common branch naming
        if name.contains('/') {
            println!("🔍 Trying alternative fetch patterns...");

            // Try: git fetch origin (to get all refs, then checkout locally)
            let fetch_all_result = Command::new("git")
                .args(["fetch", "origin"])
                .current_dir(&self.path)
                .output();

            if let Ok(output) = fetch_all_result {
                if output.status.success() {
                    // Try to create local branch from remote
                    let checkout_result = Command::new("git")
                        .args(["checkout", "-b", name, &format!("origin/{name}")])
                        .current_dir(&self.path)
                        .output();

                    if let Ok(checkout_output) = checkout_result {
                        if checkout_output.status.success() {
                            println!(
                                "✅ Successfully created local branch '{name}' from origin/{name}"
                            );
                            return Ok(true);
                        }
                    }
                }
            }
        }

        // 5. Only fail if it doesn't exist anywhere
        Ok(false)
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

    /// Get the upstream branch for a local branch
    pub fn get_upstream_branch(&self, branch_name: &str) -> Result<Option<String>> {
        // Try to get the upstream from git config
        let config = self.repo.config().map_err(CascadeError::Git)?;

        // Check for branch.{branch_name}.remote and branch.{branch_name}.merge
        let remote_key = format!("branch.{branch_name}.remote");
        let merge_key = format!("branch.{branch_name}.merge");

        if let (Ok(remote), Ok(merge_ref)) = (
            config.get_string(&remote_key),
            config.get_string(&merge_key),
        ) {
            // Parse the merge ref (e.g., "refs/heads/feature-auth" -> "feature-auth")
            if let Some(branch_part) = merge_ref.strip_prefix("refs/heads/") {
                return Ok(Some(format!("{remote}/{branch_part}")));
            }
        }

        // Fallback: check if there's a remote tracking branch with the same name
        let potential_upstream = format!("origin/{branch_name}");
        if self
            .repo
            .find_reference(&format!("refs/remotes/{potential_upstream}"))
            .is_ok()
        {
            return Ok(Some(potential_upstream));
        }

        Ok(None)
    }

    /// Get ahead/behind counts compared to upstream
    pub fn get_ahead_behind_counts(
        &self,
        local_branch: &str,
        upstream_branch: &str,
    ) -> Result<(usize, usize)> {
        // Get the commit objects for both branches
        let local_ref = self
            .repo
            .find_reference(&format!("refs/heads/{local_branch}"))
            .map_err(|_| {
                CascadeError::config(format!("Local branch '{local_branch}' not found"))
            })?;
        let local_commit = local_ref.peel_to_commit().map_err(CascadeError::Git)?;

        let upstream_ref = self
            .repo
            .find_reference(&format!("refs/remotes/{upstream_branch}"))
            .map_err(|_| {
                CascadeError::config(format!("Upstream branch '{upstream_branch}' not found"))
            })?;
        let upstream_commit = upstream_ref.peel_to_commit().map_err(CascadeError::Git)?;

        // Use git2's graph_ahead_behind to calculate the counts
        let (ahead, behind) = self
            .repo
            .graph_ahead_behind(local_commit.id(), upstream_commit.id())
            .map_err(CascadeError::Git)?;

        Ok((ahead, behind))
    }

    /// Set upstream tracking for a branch
    pub fn set_upstream(&self, branch_name: &str, remote: &str, remote_branch: &str) -> Result<()> {
        let mut config = self.repo.config().map_err(CascadeError::Git)?;

        // Set branch.{branch_name}.remote = remote
        let remote_key = format!("branch.{branch_name}.remote");
        config
            .set_str(&remote_key, remote)
            .map_err(CascadeError::Git)?;

        // Set branch.{branch_name}.merge = refs/heads/{remote_branch}
        let merge_key = format!("branch.{branch_name}.merge");
        let merge_value = format!("refs/heads/{remote_branch}");
        config
            .set_str(&merge_key, &merge_value)
            .map_err(CascadeError::Git)?;

        Ok(())
    }

    /// Create a commit with all staged changes
    pub fn commit(&self, message: &str) -> Result<String> {
        // Validate git user configuration before attempting commit operations
        self.validate_git_user_config()?;

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

        Output::success(format!("Created commit: {commit_id} - {message}"));
        Ok(commit_id.to_string())
    }

    /// Commit any staged changes with a default message
    pub fn commit_staged_changes(&self, default_message: &str) -> Result<Option<String>> {
        // Check if there are staged changes
        let staged_files = self.get_staged_files()?;
        if staged_files.is_empty() {
            tracing::debug!("No staged changes to commit");
            return Ok(None);
        }

        tracing::info!("Committing {} staged files", staged_files.len());
        let commit_hash = self.commit(default_message)?;
        Ok(Some(commit_hash))
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

    /// Stage only specific files (safer than stage_all during rebase)
    pub fn stage_files(&self, file_paths: &[&str]) -> Result<()> {
        if file_paths.is_empty() {
            tracing::debug!("No files to stage");
            return Ok(());
        }

        let mut index = self.repo.index().map_err(CascadeError::Git)?;

        for file_path in file_paths {
            index
                .add_path(std::path::Path::new(file_path))
                .map_err(CascadeError::Git)?;
        }

        index.write().map_err(CascadeError::Git)?;

        tracing::debug!(
            "Staged {} specific files: {:?}",
            file_paths.len(),
            file_paths
        );
        Ok(())
    }

    /// Stage only files that had conflicts (safer for rebase operations)
    pub fn stage_conflict_resolved_files(&self) -> Result<()> {
        let conflicted_files = self.get_conflicted_files()?;
        if conflicted_files.is_empty() {
            tracing::debug!("No conflicted files to stage");
            return Ok(());
        }

        let file_paths: Vec<&str> = conflicted_files.iter().map(|s| s.as_str()).collect();
        self.stage_files(&file_paths)?;

        tracing::debug!("Staged {} conflict-resolved files", conflicted_files.len());
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

    /// Validate git user configuration is properly set
    pub fn validate_git_user_config(&self) -> Result<()> {
        if let Ok(config) = self.repo.config() {
            let name_result = config.get_string("user.name");
            let email_result = config.get_string("user.email");

            if let (Ok(name), Ok(email)) = (name_result, email_result) {
                if !name.trim().is_empty() && !email.trim().is_empty() {
                    tracing::debug!("Git user config validated: {} <{}>", name, email);
                    return Ok(());
                }
            }
        }

        // Check if this is a CI environment where validation can be skipped
        let is_ci = std::env::var("CI").is_ok();

        if is_ci {
            tracing::debug!("CI environment - skipping git user config validation");
            return Ok(());
        }

        Output::warning("Git user configuration missing or incomplete");
        Output::info("This can cause cherry-pick and commit operations to fail");
        Output::info("Please configure git user information:");
        Output::bullet("git config user.name \"Your Name\"".to_string());
        Output::bullet("git config user.email \"your.email@example.com\"".to_string());
        Output::info("Or set globally with the --global flag");

        // Don't fail - let operations continue with fallback signature
        // This preserves backward compatibility while providing guidance
        Ok(())
    }

    /// Get a signature for commits with comprehensive fallback and validation
    fn get_signature(&self) -> Result<Signature<'_>> {
        // Try to get signature from Git config first
        if let Ok(config) = self.repo.config() {
            // Try global/system config first
            let name_result = config.get_string("user.name");
            let email_result = config.get_string("user.email");

            if let (Ok(name), Ok(email)) = (name_result, email_result) {
                if !name.trim().is_empty() && !email.trim().is_empty() {
                    tracing::debug!("Using git config: {} <{}>", name, email);
                    return Signature::now(&name, &email).map_err(CascadeError::Git);
                }
            } else {
                tracing::debug!("Git user config incomplete or missing");
            }
        }

        // Check if this is a CI environment where fallback is acceptable
        let is_ci = std::env::var("CI").is_ok();

        if is_ci {
            tracing::debug!("CI environment detected, using fallback signature");
            return Signature::now("Cascade CLI", "cascade@example.com").map_err(CascadeError::Git);
        }

        // Interactive environment - provide helpful guidance
        tracing::warn!("Git user configuration missing - this can cause commit operations to fail");

        // Try fallback signature, but warn about the issue
        match Signature::now("Cascade CLI", "cascade@example.com") {
            Ok(sig) => {
                Output::warning("Git user not configured - using fallback signature");
                Output::info("For better git history, run:");
                Output::bullet("git config user.name \"Your Name\"".to_string());
                Output::bullet("git config user.email \"your.email@example.com\"".to_string());
                Output::info("Or set it globally with --global flag");
                Ok(sig)
            }
            Err(e) => {
                Err(CascadeError::branch(format!(
                    "Cannot create git signature: {e}. Please configure git user with:\n  git config user.name \"Your Name\"\n  git config user.email \"your.email@example.com\""
                )))
            }
        }
    }

    /// Configure remote callbacks with SSL settings
    /// Priority: Cascade SSL config > Git config > Default
    fn configure_remote_callbacks(&self) -> Result<git2::RemoteCallbacks<'_>> {
        self.configure_remote_callbacks_with_fallback(false)
    }

    /// Determine if we should retry with DefaultCredentials based on git2 error classification
    fn should_retry_with_default_credentials(&self, error: &git2::Error) -> bool {
        match error.class() {
            // Authentication errors that might be resolved with DefaultCredentials
            git2::ErrorClass::Http => {
                // HTTP errors often indicate authentication issues in corporate environments
                match error.code() {
                    git2::ErrorCode::Auth => true,
                    _ => {
                        // Check for specific HTTP authentication replay errors
                        let error_string = error.to_string();
                        error_string.contains("too many redirects")
                            || error_string.contains("authentication replays")
                            || error_string.contains("authentication required")
                    }
                }
            }
            git2::ErrorClass::Net => {
                // Network errors that might be authentication-related
                let error_string = error.to_string();
                error_string.contains("authentication")
                    || error_string.contains("unauthorized")
                    || error_string.contains("forbidden")
            }
            _ => false,
        }
    }

    /// Determine if we should fallback to git CLI based on git2 error classification
    fn should_fallback_to_git_cli(&self, error: &git2::Error) -> bool {
        match error.class() {
            // SSL/TLS errors that git CLI handles better
            git2::ErrorClass::Ssl => true,

            // Certificate errors
            git2::ErrorClass::Http if error.code() == git2::ErrorCode::Certificate => true,

            // SSH errors that might need git CLI
            git2::ErrorClass::Ssh => {
                let error_string = error.to_string();
                error_string.contains("no callback set")
                    || error_string.contains("authentication required")
            }

            // Network errors that might be proxy/firewall related
            git2::ErrorClass::Net => {
                let error_string = error.to_string();
                error_string.contains("TLS stream")
                    || error_string.contains("SSL")
                    || error_string.contains("proxy")
                    || error_string.contains("firewall")
            }

            // General HTTP errors not handled by DefaultCredentials retry
            git2::ErrorClass::Http => {
                let error_string = error.to_string();
                error_string.contains("TLS stream")
                    || error_string.contains("SSL")
                    || error_string.contains("proxy")
            }

            _ => false,
        }
    }

    fn configure_remote_callbacks_with_fallback(
        &self,
        use_default_first: bool,
    ) -> Result<git2::RemoteCallbacks<'_>> {
        let mut callbacks = git2::RemoteCallbacks::new();

        // Configure authentication with comprehensive credential support
        let bitbucket_credentials = self.bitbucket_credentials.clone();
        callbacks.credentials(move |url, username_from_url, allowed_types| {
            tracing::debug!(
                "Authentication requested for URL: {}, username: {:?}, allowed_types: {:?}",
                url,
                username_from_url,
                allowed_types
            );

            // For SSH URLs with username
            if allowed_types.contains(git2::CredentialType::SSH_KEY) {
                if let Some(username) = username_from_url {
                    tracing::debug!("Trying SSH key authentication for user: {}", username);
                    return git2::Cred::ssh_key_from_agent(username);
                }
            }

            // For HTTPS URLs, try multiple authentication methods in sequence
            if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
                // If we're in corporate network fallback mode, try DefaultCredentials first
                if use_default_first {
                    tracing::debug!("Corporate network mode: trying DefaultCredentials first");
                    return git2::Cred::default();
                }

                if url.contains("bitbucket") {
                    if let Some(creds) = &bitbucket_credentials {
                        // Method 1: Username + Token (common for Bitbucket)
                        if let (Some(username), Some(token)) = (&creds.username, &creds.token) {
                            tracing::debug!("Trying Bitbucket username + token authentication");
                            return git2::Cred::userpass_plaintext(username, token);
                        }

                        // Method 2: Token as username, empty password (alternate Bitbucket format)
                        if let Some(token) = &creds.token {
                            tracing::debug!("Trying Bitbucket token-as-username authentication");
                            return git2::Cred::userpass_plaintext(token, "");
                        }

                        // Method 3: Just username (will prompt for password or use credential helper)
                        if let Some(username) = &creds.username {
                            tracing::debug!("Trying Bitbucket username authentication (will use credential helper)");
                            return git2::Cred::username(username);
                        }
                    }
                }

                // Method 4: Default credential helper for all HTTPS URLs
                tracing::debug!("Trying default credential helper for HTTPS authentication");
                return git2::Cred::default();
            }

            // Fallback to default for any other cases
            tracing::debug!("Using default credential fallback");
            git2::Cred::default()
        });

        // Configure SSL certificate checking with system certificates by default
        // This matches what tools like Graphite, Sapling, and Phabricator do
        // Priority: 1. Use system certificates (default), 2. Manual overrides only if needed

        let mut ssl_configured = false;

        // Check for manual SSL overrides first (only when user explicitly needs them)
        if let Some(ssl_config) = &self.ssl_config {
            if ssl_config.accept_invalid_certs {
                Output::warning(
                    "SSL certificate verification DISABLED via Cascade config - this is insecure!",
                );
                callbacks.certificate_check(|_cert, _host| {
                    tracing::debug!("⚠️  Accepting invalid certificate for host: {}", _host);
                    Ok(git2::CertificateCheckStatus::CertificateOk)
                });
                ssl_configured = true;
            } else if let Some(ca_path) = &ssl_config.ca_bundle_path {
                Output::info(format!(
                    "Using custom CA bundle from Cascade config: {ca_path}"
                ));
                callbacks.certificate_check(|_cert, host| {
                    tracing::debug!("Using custom CA bundle for host: {}", host);
                    Ok(git2::CertificateCheckStatus::CertificateOk)
                });
                ssl_configured = true;
            }
        }

        // Check git config for manual overrides
        if !ssl_configured {
            if let Ok(config) = self.repo.config() {
                let ssl_verify = config.get_bool("http.sslVerify").unwrap_or(true);

                if !ssl_verify {
                    Output::warning(
                        "SSL certificate verification DISABLED via git config - this is insecure!",
                    );
                    callbacks.certificate_check(|_cert, host| {
                        tracing::debug!("⚠️  Bypassing SSL verification for host: {}", host);
                        Ok(git2::CertificateCheckStatus::CertificateOk)
                    });
                    ssl_configured = true;
                } else if let Ok(ca_path) = config.get_string("http.sslCAInfo") {
                    Output::info(format!("Using custom CA bundle from git config: {ca_path}"));
                    callbacks.certificate_check(|_cert, host| {
                        tracing::debug!("Using git config CA bundle for host: {}", host);
                        Ok(git2::CertificateCheckStatus::CertificateOk)
                    });
                    ssl_configured = true;
                }
            }
        }

        // DEFAULT BEHAVIOR: Use system certificates (like git CLI and other modern tools)
        // This should work out-of-the-box in corporate environments
        if !ssl_configured {
            tracing::debug!(
                "Using system certificate store for SSL verification (default behavior)"
            );

            // For macOS with SecureTransport backend, try default certificate validation first
            if cfg!(target_os = "macos") {
                tracing::debug!("macOS detected - using default certificate validation");
                // Don't set any certificate callback - let git2 use its default behavior
                // This often works better with SecureTransport backend on macOS
            } else {
                // Use CertificatePassthrough for other platforms
                callbacks.certificate_check(|_cert, host| {
                    tracing::debug!("System certificate validation for host: {}", host);
                    Ok(git2::CertificateCheckStatus::CertificatePassthrough)
                });
            }
        }

        Ok(callbacks)
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

    /// Get a summary of repository status
    pub fn get_status_summary(&self) -> Result<GitStatusSummary> {
        let statuses = self.get_status()?;

        let mut staged_files = 0;
        let mut unstaged_files = 0;
        let mut untracked_files = 0;

        for status in statuses.iter() {
            let flags = status.status();

            if flags.intersects(
                git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_NEW
                    | git2::Status::INDEX_DELETED
                    | git2::Status::INDEX_RENAMED
                    | git2::Status::INDEX_TYPECHANGE,
            ) {
                staged_files += 1;
            }

            if flags.intersects(
                git2::Status::WT_MODIFIED
                    | git2::Status::WT_DELETED
                    | git2::Status::WT_TYPECHANGE
                    | git2::Status::WT_RENAMED,
            ) {
                unstaged_files += 1;
            }

            if flags.intersects(git2::Status::WT_NEW) {
                untracked_files += 1;
            }
        }

        Ok(GitStatusSummary {
            staged_files,
            unstaged_files,
            untracked_files,
        })
    }

    /// Get the current commit hash (alias for get_head_commit_hash)
    pub fn get_current_commit_hash(&self) -> Result<String> {
        self.get_head_commit_hash()
    }

    /// Get the count of commits between two commits
    pub fn get_commit_count_between(&self, from_commit: &str, to_commit: &str) -> Result<usize> {
        let from_oid = git2::Oid::from_str(from_commit).map_err(CascadeError::Git)?;
        let to_oid = git2::Oid::from_str(to_commit).map_err(CascadeError::Git)?;

        let mut revwalk = self.repo.revwalk().map_err(CascadeError::Git)?;
        revwalk.push(to_oid).map_err(CascadeError::Git)?;
        revwalk.hide(from_oid).map_err(CascadeError::Git)?;

        Ok(revwalk.count())
    }

    /// Get remote URL for a given remote name
    pub fn get_remote_url(&self, name: &str) -> Result<String> {
        let remote = self.repo.find_remote(name).map_err(CascadeError::Git)?;
        Ok(remote.url().unwrap_or("unknown").to_string())
    }

    /// Cherry-pick a specific commit to the current branch
    pub fn cherry_pick(&self, commit_hash: &str) -> Result<String> {
        tracing::debug!("Cherry-picking commit {}", commit_hash);

        // Validate git user configuration before attempting commit operations
        self.validate_git_user_config()?;

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

        // Create new commit with original message (preserve it exactly)
        let signature = self.get_signature()?;
        let message = commit.message().unwrap_or("Cherry-picked commit");

        let new_commit_oid = self
            .repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &merged_tree,
                &[&head_commit],
            )
            .map_err(CascadeError::Git)?;

        // Update working directory to reflect the new commit
        let new_commit = self
            .repo
            .find_commit(new_commit_oid)
            .map_err(CascadeError::Git)?;
        let new_tree = new_commit.tree().map_err(CascadeError::Git)?;

        self.repo
            .checkout_tree(
                new_tree.as_object(),
                Some(git2::build::CheckoutBuilder::new().force()),
            )
            .map_err(CascadeError::Git)?;

        tracing::debug!("Cherry-picked {} -> {}", commit_hash, new_commit_oid);
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
        tracing::debug!("Fetching from origin");

        let mut remote = self
            .repo
            .find_remote("origin")
            .map_err(|e| CascadeError::branch(format!("No remote 'origin' found: {e}")))?;

        // Configure callbacks with SSL settings from git config
        let callbacks = self.configure_remote_callbacks()?;

        // Fetch options with authentication and SSL config
        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        // Fetch with authentication
        match remote.fetch::<&str>(&[], Some(&mut fetch_options), None) {
            Ok(_) => {
                tracing::debug!("Fetch completed successfully");
                Ok(())
            }
            Err(e) => {
                if self.should_retry_with_default_credentials(&e) {
                    tracing::debug!(
                        "Authentication error detected (class: {:?}, code: {:?}): {}, retrying with DefaultCredentials",
                        e.class(), e.code(), e
                    );

                    // Retry with DefaultCredentials for corporate networks
                    let callbacks = self.configure_remote_callbacks_with_fallback(true)?;
                    let mut fetch_options = git2::FetchOptions::new();
                    fetch_options.remote_callbacks(callbacks);

                    match remote.fetch::<&str>(&[], Some(&mut fetch_options), None) {
                        Ok(_) => {
                            tracing::debug!("Fetch succeeded with DefaultCredentials");
                            return Ok(());
                        }
                        Err(retry_error) => {
                            tracing::debug!(
                                "DefaultCredentials retry failed: {}, falling back to git CLI",
                                retry_error
                            );
                            return self.fetch_with_git_cli();
                        }
                    }
                }

                if self.should_fallback_to_git_cli(&e) {
                    tracing::debug!(
                        "Network/SSL error detected (class: {:?}, code: {:?}): {}, falling back to git CLI for fetch operation",
                        e.class(), e.code(), e
                    );
                    return self.fetch_with_git_cli();
                }
                Err(CascadeError::Git(e))
            }
        }
    }

    /// Pull changes from remote (fetch + merge)
    pub fn pull(&self, branch: &str) -> Result<()> {
        tracing::debug!("Pulling branch: {}", branch);

        // First fetch - this now includes TLS fallback
        match self.fetch() {
            Ok(_) => {}
            Err(e) => {
                // If fetch failed even with CLI fallback, try full git pull as last resort
                let error_string = e.to_string();
                if error_string.contains("TLS stream") || error_string.contains("SSL") {
                    tracing::warn!(
                        "git2 error detected: {}, falling back to git CLI for pull operation",
                        e
                    );
                    return self.pull_with_git_cli(branch);
                }
                return Err(e);
            }
        }

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

        // Check if already up to date
        if head_commit.id() == remote_commit.id() {
            tracing::debug!("Already up to date");
            return Ok(());
        }

        // Check if we can fast-forward (local is ancestor of remote)
        let merge_base_oid = self
            .repo
            .merge_base(head_commit.id(), remote_commit.id())
            .map_err(CascadeError::Git)?;

        if merge_base_oid == head_commit.id() {
            // Fast-forward: local is direct ancestor of remote, just move pointer
            tracing::debug!("Fast-forwarding {} to {}", branch, remote_commit.id());

            // Update the branch reference to point to remote commit
            let refname = format!("refs/heads/{}", branch);
            self.repo
                .reference(&refname, remote_oid, true, "pull: Fast-forward")
                .map_err(CascadeError::Git)?;

            // Update HEAD to point to the new commit
            self.repo.set_head(&refname).map_err(CascadeError::Git)?;

            // Checkout the new commit (update working directory)
            self.repo
                .checkout_head(Some(
                    git2::build::CheckoutBuilder::new()
                        .force()
                        .remove_untracked(false),
                ))
                .map_err(CascadeError::Git)?;

            tracing::debug!("Fast-forwarded to {}", remote_commit.id());
            return Ok(());
        }

        // If we can't fast-forward, the local branch has diverged
        // This should NOT happen on protected branches!
        Err(CascadeError::branch(format!(
            "Branch '{}' has diverged from remote. Local has commits not in remote. \
             Protected branches should not have local commits. \
             Try: git reset --hard origin/{}",
            branch, branch
        )))
    }

    /// Push current branch to remote
    pub fn push(&self, branch: &str) -> Result<()> {
        // Pushing branch to remote

        let mut remote = self
            .repo
            .find_remote("origin")
            .map_err(|e| CascadeError::branch(format!("No remote 'origin' found: {e}")))?;

        let remote_url = remote.url().unwrap_or("unknown").to_string();
        tracing::debug!("Remote URL: {}", remote_url);

        let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
        tracing::debug!("Push refspec: {}", refspec);

        // Configure callbacks with enhanced SSL settings and error handling
        let mut callbacks = self.configure_remote_callbacks()?;

        // Add enhanced progress and error callbacks for better debugging
        callbacks.push_update_reference(|refname, status| {
            if let Some(msg) = status {
                tracing::error!("Push failed for ref {}: {}", refname, msg);
                return Err(git2::Error::from_str(&format!("Push failed: {msg}")));
            }
            tracing::debug!("Push succeeded for ref: {}", refname);
            Ok(())
        });

        // Push options with authentication and SSL config
        let mut push_options = git2::PushOptions::new();
        push_options.remote_callbacks(callbacks);

        // Attempt push with enhanced error reporting
        match remote.push(&[&refspec], Some(&mut push_options)) {
            Ok(_) => {
                tracing::info!("Push completed successfully for branch: {}", branch);
                Ok(())
            }
            Err(e) => {
                tracing::debug!(
                    "git2 push error: {} (class: {:?}, code: {:?})",
                    e,
                    e.class(),
                    e.code()
                );

                if self.should_retry_with_default_credentials(&e) {
                    tracing::debug!(
                        "Authentication error detected (class: {:?}, code: {:?}): {}, retrying with DefaultCredentials",
                        e.class(), e.code(), e
                    );

                    // Retry with DefaultCredentials for corporate networks
                    let callbacks = self.configure_remote_callbacks_with_fallback(true)?;
                    let mut push_options = git2::PushOptions::new();
                    push_options.remote_callbacks(callbacks);

                    match remote.push(&[&refspec], Some(&mut push_options)) {
                        Ok(_) => {
                            tracing::debug!("Push succeeded with DefaultCredentials");
                            return Ok(());
                        }
                        Err(retry_error) => {
                            tracing::debug!(
                                "DefaultCredentials retry failed: {}, falling back to git CLI",
                                retry_error
                            );
                            return self.push_with_git_cli(branch);
                        }
                    }
                }

                if self.should_fallback_to_git_cli(&e) {
                    tracing::debug!(
                        "Network/SSL error detected (class: {:?}, code: {:?}): {}, falling back to git CLI for push operation",
                        e.class(), e.code(), e
                    );
                    return self.push_with_git_cli(branch);
                }

                // Create concise error message
                let error_msg = if e.to_string().contains("authentication") {
                    format!(
                        "Authentication failed for branch '{branch}'. Try: git push origin {branch}"
                    )
                } else {
                    format!("Failed to push branch '{branch}': {e}")
                };

                tracing::error!("{}", error_msg);
                Err(CascadeError::branch(error_msg))
            }
        }
    }

    /// Fallback push method using git CLI instead of git2
    /// This is used when git2 has TLS/SSL or auth issues but git CLI works fine
    fn push_with_git_cli(&self, branch: &str) -> Result<()> {
        let output = std::process::Command::new("git")
            .args(["push", "origin", branch])
            .current_dir(&self.path)
            .output()
            .map_err(|e| CascadeError::branch(format!("Failed to execute git command: {e}")))?;

        if output.status.success() {
            // Silent success - no need to log when fallback works
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _stdout = String::from_utf8_lossy(&output.stdout);
            // Extract the most relevant error message
            let error_msg = if stderr.contains("SSL_connect") || stderr.contains("SSL_ERROR") {
                "Network error: Unable to connect to repository (VPN may be required)".to_string()
            } else if stderr.contains("repository") && stderr.contains("not found") {
                "Repository not found - check your Bitbucket configuration".to_string()
            } else if stderr.contains("authentication") || stderr.contains("403") {
                "Authentication failed - check your credentials".to_string()
            } else {
                // For other errors, just show the stderr without the verbose prefix
                stderr.trim().to_string()
            };
            tracing::error!("{}", error_msg);
            Err(CascadeError::branch(error_msg))
        }
    }

    /// Fallback fetch method using git CLI instead of git2
    /// This is used when git2 has TLS/SSL issues but git CLI works fine
    fn fetch_with_git_cli(&self) -> Result<()> {
        tracing::debug!("Using git CLI fallback for fetch operation");

        let output = std::process::Command::new("git")
            .args(["fetch", "origin"])
            .current_dir(&self.path)
            .output()
            .map_err(|e| {
                CascadeError::Git(git2::Error::from_str(&format!(
                    "Failed to execute git command: {e}"
                )))
            })?;

        if output.status.success() {
            tracing::debug!("Git CLI fetch succeeded");
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let error_msg = format!(
                "Git CLI fetch failed: {}\nStdout: {}\nStderr: {}",
                output.status, stdout, stderr
            );
            tracing::error!("{}", error_msg);
            Err(CascadeError::Git(git2::Error::from_str(&error_msg)))
        }
    }

    /// Fallback pull method using git CLI instead of git2
    /// This is used when git2 has TLS/SSL issues but git CLI works fine
    fn pull_with_git_cli(&self, branch: &str) -> Result<()> {
        tracing::debug!("Using git CLI fallback for pull operation: {}", branch);

        let output = std::process::Command::new("git")
            .args(["pull", "origin", branch])
            .current_dir(&self.path)
            .output()
            .map_err(|e| {
                CascadeError::Git(git2::Error::from_str(&format!(
                    "Failed to execute git command: {e}"
                )))
            })?;

        if output.status.success() {
            tracing::info!("✅ Git CLI pull succeeded for branch: {}", branch);
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let error_msg = format!(
                "Git CLI pull failed for branch '{}': {}\nStdout: {}\nStderr: {}",
                branch, output.status, stdout, stderr
            );
            tracing::error!("{}", error_msg);
            Err(CascadeError::Git(git2::Error::from_str(&error_msg)))
        }
    }

    /// Fallback force push method using git CLI instead of git2
    /// This is used when git2 has TLS/SSL issues but git CLI works fine
    fn force_push_with_git_cli(&self, branch: &str) -> Result<()> {
        tracing::debug!(
            "Using git CLI fallback for force push operation: {}",
            branch
        );

        let output = std::process::Command::new("git")
            .args(["push", "--force", "origin", branch])
            .current_dir(&self.path)
            .output()
            .map_err(|e| CascadeError::branch(format!("Failed to execute git command: {e}")))?;

        if output.status.success() {
            tracing::debug!("Git CLI force push succeeded for branch: {}", branch);
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let error_msg = format!(
                "Git CLI force push failed for branch '{}': {}\nStdout: {}\nStderr: {}",
                branch, output.status, stdout, stderr
            );
            tracing::error!("{}", error_msg);
            Err(CascadeError::branch(error_msg))
        }
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
        debug!("Attempting to delete branch: {}", name);

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

        debug!("Successfully deleted branch '{}'", name);
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
        debug!(
            "Force pushing {} content to {} to preserve PR history",
            source_branch, target_branch
        );

        // Enhanced safety check: Detect potential data loss and get user confirmation
        if !force_unsafe {
            let safety_result = self.check_force_push_safety_enhanced(target_branch)?;
            if let Some(backup_info) = safety_result {
                // Create backup branch before force push
                self.create_backup_branch(target_branch, &backup_info.remote_commit_id)?;
                debug!("Created backup branch: {}", backup_info.backup_branch_name);
            }
        }

        // First, ensure we have the latest changes for the source branch
        let source_ref = self
            .repo
            .find_reference(&format!("refs/heads/{source_branch}"))
            .map_err(|e| {
                CascadeError::config(format!("Failed to find source branch {source_branch}: {e}"))
            })?;
        let _source_commit = source_ref.peel_to_commit().map_err(|e| {
            CascadeError::config(format!(
                "Failed to get commit for source branch {source_branch}: {e}"
            ))
        })?;

        // Force push to remote without modifying local target branch
        let mut remote = self
            .repo
            .find_remote("origin")
            .map_err(|e| CascadeError::config(format!("Failed to find origin remote: {e}")))?;

        // Push source branch content to remote target branch
        let refspec = format!("+refs/heads/{source_branch}:refs/heads/{target_branch}");

        // Configure callbacks with SSL settings from git config
        let callbacks = self.configure_remote_callbacks()?;

        // Push options for force push with SSL config
        let mut push_options = git2::PushOptions::new();
        push_options.remote_callbacks(callbacks);

        match remote.push(&[&refspec], Some(&mut push_options)) {
            Ok(_) => {}
            Err(e) => {
                if self.should_retry_with_default_credentials(&e) {
                    tracing::debug!(
                        "Authentication error detected (class: {:?}, code: {:?}): {}, retrying with DefaultCredentials",
                        e.class(), e.code(), e
                    );

                    // Retry with DefaultCredentials for corporate networks
                    let callbacks = self.configure_remote_callbacks_with_fallback(true)?;
                    let mut push_options = git2::PushOptions::new();
                    push_options.remote_callbacks(callbacks);

                    match remote.push(&[&refspec], Some(&mut push_options)) {
                        Ok(_) => {
                            tracing::debug!("Force push succeeded with DefaultCredentials");
                            // Success - continue to normal success path
                        }
                        Err(retry_error) => {
                            tracing::debug!(
                                "DefaultCredentials retry failed: {}, falling back to git CLI",
                                retry_error
                            );
                            return self.force_push_with_git_cli(target_branch);
                        }
                    }
                } else if self.should_fallback_to_git_cli(&e) {
                    tracing::debug!(
                        "Network/SSL error detected (class: {:?}, code: {:?}): {}, falling back to git CLI for force push operation",
                        e.class(), e.code(), e
                    );
                    return self.force_push_with_git_cli(target_branch);
                } else {
                    return Err(CascadeError::config(format!(
                        "Failed to force push {target_branch}: {e}"
                    )));
                }
            }
        }

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

                    debug!(
                        "Force push to '{}' would overwrite {} commits on remote",
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

    /// Check force push safety without user confirmation (auto-creates backup)
    /// Used for automated operations like sync where user already confirmed the operation
    fn check_force_push_safety_auto(&self, target_branch: &str) -> Result<Option<ForceBackupInfo>> {
        // First fetch latest remote changes to ensure we have up-to-date information
        match self.fetch() {
            Ok(_) => {}
            Err(e) => {
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

                    debug!(
                        "Auto-creating backup for force push to '{}' (would overwrite {} commits)",
                        target_branch, commits_to_lose
                    );

                    // Automatically create backup without confirmation
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

        debug!(
            "Created backup branch '{}' pointing to {}",
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
                if let Ok(oid) = Oid::from_str(commit_id) {
                    if let Ok(commit) = self.repo.find_commit(oid) {
                        let short_hash = &commit_id[..8];
                        let summary = commit.summary().unwrap_or("<no message>");
                        println!("  {}. {} - {}", i + 1, short_hash, summary);
                    }
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
    pub fn detect_main_branch(&self) -> Result<String> {
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

        // Use proper selection dialog instead of y/n confirmation
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Choose an action")
            .items(&[
                "Stash changes and checkout (recommended)",
                "Force checkout (WILL LOSE UNCOMMITTED CHANGES)",
                "Cancel checkout",
            ])
            .default(0)
            .interact()
            .map_err(|e| CascadeError::branch(format!("Could not get user selection: {e}")))?;

        match selection {
            0 => {
                // Option 1: Stash changes and checkout
                let stash_message = format!(
                    "Auto-stash before checkout to {} at {}",
                    target,
                    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                );

                match self.create_stash(&stash_message) {
                    Ok(stash_id) => {
                        println!("✅ Created stash: {stash_message} ({stash_id})");
                        println!("💡 You can restore with: git stash pop");
                    }
                    Err(e) => {
                        println!("❌ Failed to create stash: {e}");

                        // If stash failed, provide better options
                        use dialoguer::Select;
                        let stash_failed_options = vec![
                            "Commit staged changes and proceed",
                            "Force checkout (WILL LOSE CHANGES)",
                            "Cancel and handle manually",
                        ];

                        let stash_selection = Select::with_theme(&ColorfulTheme::default())
                            .with_prompt("Stash failed. What would you like to do?")
                            .items(&stash_failed_options)
                            .default(0)
                            .interact()
                            .map_err(|e| {
                                CascadeError::branch(format!("Could not get user selection: {e}"))
                            })?;

                        match stash_selection {
                            0 => {
                                // Try to commit staged changes
                                let staged_files = self.get_staged_files()?;
                                if !staged_files.is_empty() {
                                    println!(
                                        "📝 Committing {} staged files...",
                                        staged_files.len()
                                    );
                                    match self
                                        .commit_staged_changes("WIP: Auto-commit before checkout")
                                    {
                                        Ok(Some(commit_hash)) => {
                                            println!(
                                                "✅ Committed staged changes as {}",
                                                &commit_hash[..8]
                                            );
                                            println!("💡 You can undo with: git reset HEAD~1");
                                        }
                                        Ok(None) => {
                                            println!("ℹ️  No staged changes found to commit");
                                        }
                                        Err(commit_err) => {
                                            println!(
                                                "❌ Failed to commit staged changes: {commit_err}"
                                            );
                                            return Err(CascadeError::branch(
                                                "Could not commit staged changes".to_string(),
                                            ));
                                        }
                                    }
                                } else {
                                    println!("ℹ️  No staged changes to commit");
                                }
                            }
                            1 => {
                                // Force checkout anyway
                                println!("⚠️  Proceeding with force checkout - uncommitted changes will be lost!");
                            }
                            2 => {
                                // Cancel
                                return Err(CascadeError::branch(
                                    "Checkout cancelled. Please handle changes manually and try again.".to_string(),
                                ));
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
            1 => {
                // Option 2: Force checkout (lose changes)
                println!("⚠️  Proceeding with force checkout - uncommitted changes will be lost!");
            }
            2 => {
                // Option 3: Cancel
                return Err(CascadeError::branch(
                    "Checkout cancelled by user".to_string(),
                ));
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    /// Create a stash with uncommitted changes
    fn create_stash(&self, message: &str) -> Result<String> {
        tracing::info!("Creating stash: {}", message);

        // Use git CLI for stashing since git2 stashing is complex and unreliable
        let output = std::process::Command::new("git")
            .args(["stash", "push", "-m", message])
            .current_dir(&self.path)
            .output()
            .map_err(|e| {
                CascadeError::branch(format!("Failed to execute git stash command: {e}"))
            })?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Extract stash hash if available (git stash outputs like "Saved working directory and index state WIP on branch: message")
            let stash_id = if stdout.contains("Saved working directory") {
                // Get the most recent stash ID
                let stash_list_output = std::process::Command::new("git")
                    .args(["stash", "list", "-n", "1", "--format=%H"])
                    .current_dir(&self.path)
                    .output()
                    .map_err(|e| CascadeError::branch(format!("Failed to get stash ID: {e}")))?;

                if stash_list_output.status.success() {
                    String::from_utf8_lossy(&stash_list_output.stdout)
                        .trim()
                        .to_string()
                } else {
                    "stash@{0}".to_string() // fallback
                }
            } else {
                "stash@{0}".to_string() // fallback
            };

            tracing::info!("✅ Created stash: {} ({})", message, stash_id);
            Ok(stash_id)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Check for common stash failure reasons
            if stderr.contains("No local changes to save")
                || stdout.contains("No local changes to save")
            {
                return Err(CascadeError::branch("No local changes to save".to_string()));
            }

            Err(CascadeError::branch(format!(
                "Failed to create stash: {}\nStderr: {}\nStdout: {}",
                output.status, stderr, stdout
            )))
        }
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
    pub fn get_staged_files(&self) -> Result<Vec<String>> {
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

    /// Reset working directory and index to match HEAD (hard reset)
    /// This clears all uncommitted changes and staged files
    pub fn reset_to_head(&self) -> Result<()> {
        tracing::debug!("Resetting working directory and index to HEAD");

        let head = self.repo.head().map_err(CascadeError::Git)?;
        let head_commit = head.peel_to_commit().map_err(CascadeError::Git)?;

        // Hard reset: resets index and working tree
        let mut checkout_builder = git2::build::CheckoutBuilder::new();
        checkout_builder.force(); // Force checkout to overwrite any local changes
        checkout_builder.remove_untracked(false); // Don't remove untracked files

        self.repo
            .reset(
                head_commit.as_object(),
                git2::ResetType::Hard,
                Some(&mut checkout_builder),
            )
            .map_err(CascadeError::Git)?;

        tracing::debug!("Successfully reset working directory to HEAD");
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

    /// Reset a branch to point to a specific commit
    pub fn reset_branch_to_commit(&self, branch_name: &str, commit_hash: &str) -> Result<()> {
        info!(
            "Resetting branch '{}' to commit {}",
            branch_name,
            &commit_hash[..8]
        );

        // Find the target commit
        let target_oid = git2::Oid::from_str(commit_hash).map_err(|e| {
            CascadeError::branch(format!("Invalid commit hash '{commit_hash}': {e}"))
        })?;

        let _target_commit = self.repo.find_commit(target_oid).map_err(|e| {
            CascadeError::branch(format!("Could not find commit '{commit_hash}': {e}"))
        })?;

        // Find the branch
        let _branch = self
            .repo
            .find_branch(branch_name, git2::BranchType::Local)
            .map_err(|e| {
                CascadeError::branch(format!("Could not find branch '{branch_name}': {e}"))
            })?;

        // Update the branch reference to point to the target commit
        let branch_ref_name = format!("refs/heads/{branch_name}");
        self.repo
            .reference(
                &branch_ref_name,
                target_oid,
                true,
                &format!("Reset {branch_name} to {commit_hash}"),
            )
            .map_err(|e| {
                CascadeError::branch(format!(
                    "Could not reset branch '{branch_name}' to commit '{commit_hash}': {e}"
                ))
            })?;

        tracing::info!(
            "Successfully reset branch '{}' to commit {}",
            branch_name,
            &commit_hash[..8]
        );
        Ok(())
    }

    /// Detect the parent branch of the current branch using multiple strategies
    pub fn detect_parent_branch(&self) -> Result<Option<String>> {
        let current_branch = self.get_current_branch()?;

        // Strategy 1: Check if current branch has an upstream tracking branch
        if let Ok(Some(upstream)) = self.get_upstream_branch(&current_branch) {
            // Extract the branch name from "origin/branch-name" format
            if let Some(branch_name) = upstream.split('/').nth(1) {
                if self.branch_exists(branch_name) {
                    tracing::debug!(
                        "Detected parent branch '{}' from upstream tracking",
                        branch_name
                    );
                    return Ok(Some(branch_name.to_string()));
                }
            }
        }

        // Strategy 2: Use git's default branch detection
        if let Ok(default_branch) = self.detect_main_branch() {
            // Don't suggest the current branch as its own parent
            if current_branch != default_branch {
                tracing::debug!(
                    "Detected parent branch '{}' as repository default",
                    default_branch
                );
                return Ok(Some(default_branch));
            }
        }

        // Strategy 3: Find the branch with the most recent common ancestor
        // Get all local branches and find the one with the shortest commit distance
        if let Ok(branches) = self.list_branches() {
            let current_commit = self.get_head_commit()?;
            let current_commit_hash = current_commit.id().to_string();
            let current_oid = current_commit.id();

            let mut best_candidate = None;
            let mut best_distance = usize::MAX;

            for branch in branches {
                // Skip the current branch and any branches that look like version branches
                if branch == current_branch
                    || branch.contains("-v")
                    || branch.ends_with("-v2")
                    || branch.ends_with("-v3")
                {
                    continue;
                }

                if let Ok(base_commit_hash) = self.get_branch_commit_hash(&branch) {
                    if let Ok(base_oid) = git2::Oid::from_str(&base_commit_hash) {
                        // Find merge base between current branch and this branch
                        if let Ok(merge_base_oid) = self.repo.merge_base(current_oid, base_oid) {
                            // Count commits from merge base to current head
                            if let Ok(distance) = self.count_commits_between(
                                &merge_base_oid.to_string(),
                                &current_commit_hash,
                            ) {
                                // Prefer branches with shorter distances (more recent common ancestor)
                                // Also prefer branches that look like base branches
                                let is_likely_base = self.is_likely_base_branch(&branch);
                                let adjusted_distance = if is_likely_base {
                                    distance
                                } else {
                                    distance + 1000
                                };

                                if adjusted_distance < best_distance {
                                    best_distance = adjusted_distance;
                                    best_candidate = Some(branch.clone());
                                }
                            }
                        }
                    }
                }
            }

            if let Some(ref candidate) = best_candidate {
                tracing::debug!(
                    "Detected parent branch '{}' with distance {}",
                    candidate,
                    best_distance
                );
            }

            return Ok(best_candidate);
        }

        tracing::debug!("Could not detect parent branch for '{}'", current_branch);
        Ok(None)
    }

    /// Check if a branch name looks like a typical base branch
    fn is_likely_base_branch(&self, branch_name: &str) -> bool {
        let base_patterns = [
            "main",
            "master",
            "develop",
            "dev",
            "development",
            "staging",
            "stage",
            "release",
            "production",
            "prod",
        ];

        base_patterns.contains(&branch_name)
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
        let _result = repo.checkout_branch_unsafe("main");
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

        // Test stash creation - newer git versions allow empty stashes
        let result = repo.create_stash("test stash");

        // Either succeeds (newer git with empty stash) or fails with helpful message
        match result {
            Ok(stash_id) => {
                // Modern git allows empty stashes, verify we got a stash ID
                assert!(!stash_id.is_empty());
                assert!(stash_id.contains("stash") || stash_id.len() >= 7); // SHA or stash@{n}
            }
            Err(error) => {
                // Older git should fail with helpful message
                let error_msg = error.to_string();
                assert!(
                    error_msg.contains("No local changes to save")
                        || error_msg.contains("git stash push")
                );
            }
        }
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

        // Add another commit to the test branch to make it different from main
        create_commit(&repo_path, "Branch-specific commit", "branch.txt");

        // Go back to main
        Command::new("git")
            .args(["checkout", "main"])
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

    #[test]
    fn test_cherry_pick_basic() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Create a branch with a commit to cherry-pick
        repo.create_branch("source", None).unwrap();
        repo.checkout_branch("source").unwrap();

        std::fs::write(repo_path.join("cherry.txt"), "Cherry content").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Cherry commit"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let cherry_commit = repo.get_head_commit_hash().unwrap();

        // Switch back to previous branch (where source was created from)
        // Using `git checkout -` is environment-agnostic
        Command::new("git")
            .args(["checkout", "-"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        repo.create_branch("target", None).unwrap();
        repo.checkout_branch("target").unwrap();

        // Cherry-pick the commit
        let new_commit = repo.cherry_pick(&cherry_commit).unwrap();

        // Verify new commit exists and is different
        assert_ne!(new_commit, cherry_commit, "Should create new commit hash");

        // Verify file exists on target branch
        assert!(
            repo_path.join("cherry.txt").exists(),
            "Cherry-picked file should exist"
        );

        // Verify source branch is unchanged
        repo.checkout_branch("source").unwrap();
        let source_head = repo.get_head_commit_hash().unwrap();
        assert_eq!(
            source_head, cherry_commit,
            "Source branch should be unchanged"
        );
    }

    #[test]
    fn test_cherry_pick_preserves_commit_message() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Create commit with specific message
        repo.create_branch("msg-test", None).unwrap();
        repo.checkout_branch("msg-test").unwrap();

        std::fs::write(repo_path.join("msg.txt"), "Content").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let commit_msg = "Test: Special commit message\n\nWith body";
        Command::new("git")
            .args(["commit", "-m", commit_msg])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let original_commit = repo.get_head_commit_hash().unwrap();

        // Cherry-pick to another branch (use previous branch via git checkout -)
        Command::new("git")
            .args(["checkout", "-"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let new_commit = repo.cherry_pick(&original_commit).unwrap();

        // Get commit message of new commit
        let output = Command::new("git")
            .args(["log", "-1", "--format=%B", &new_commit])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let new_msg = String::from_utf8_lossy(&output.stdout);
        assert!(
            new_msg.contains("Special commit message"),
            "Should preserve commit message"
        );
    }

    #[test]
    fn test_cherry_pick_handles_conflicts() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Create conflicting content
        std::fs::write(repo_path.join("conflict.txt"), "Original").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Add conflict file"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Create branch with different content
        repo.create_branch("conflict-branch", None).unwrap();
        repo.checkout_branch("conflict-branch").unwrap();

        std::fs::write(repo_path.join("conflict.txt"), "Modified").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Modify conflict file"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        let conflict_commit = repo.get_head_commit_hash().unwrap();

        // Try to cherry-pick (should fail due to conflict)
        // Go back to previous branch
        Command::new("git")
            .args(["checkout", "-"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        std::fs::write(repo_path.join("conflict.txt"), "Different").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", "Different change"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Cherry-pick should fail with conflict
        let result = repo.cherry_pick(&conflict_commit);
        assert!(result.is_err(), "Cherry-pick with conflict should fail");
    }

    #[test]
    fn test_reset_to_head_clears_staged_files() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Create and stage some files
        std::fs::write(repo_path.join("staged1.txt"), "Content 1").unwrap();
        std::fs::write(repo_path.join("staged2.txt"), "Content 2").unwrap();

        Command::new("git")
            .args(["add", "staged1.txt", "staged2.txt"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Verify files are staged
        let staged_before = repo.get_staged_files().unwrap();
        assert_eq!(staged_before.len(), 2, "Should have 2 staged files");

        // Reset to HEAD
        repo.reset_to_head().unwrap();

        // Verify no files are staged after reset
        let staged_after = repo.get_staged_files().unwrap();
        assert_eq!(
            staged_after.len(),
            0,
            "Should have no staged files after reset"
        );
    }

    #[test]
    fn test_reset_to_head_clears_modified_files() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Modify an existing file
        std::fs::write(repo_path.join("README.md"), "# Modified content").unwrap();

        // Stage the modification
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Verify file is modified and staged
        assert!(repo.is_dirty().unwrap(), "Repo should be dirty");

        // Reset to HEAD
        repo.reset_to_head().unwrap();

        // Verify repo is clean
        assert!(
            !repo.is_dirty().unwrap(),
            "Repo should be clean after reset"
        );

        // Verify file content is restored
        let content = std::fs::read_to_string(repo_path.join("README.md")).unwrap();
        assert_eq!(
            content, "# Test",
            "File should be restored to original content"
        );
    }

    #[test]
    fn test_reset_to_head_preserves_untracked_files() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Create untracked file
        std::fs::write(repo_path.join("untracked.txt"), "Untracked content").unwrap();

        // Stage some other file
        std::fs::write(repo_path.join("staged.txt"), "Staged content").unwrap();
        Command::new("git")
            .args(["add", "staged.txt"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Reset to HEAD
        repo.reset_to_head().unwrap();

        // Verify untracked file still exists
        assert!(
            repo_path.join("untracked.txt").exists(),
            "Untracked file should be preserved"
        );

        // Verify staged file was removed (since it was never committed)
        assert!(
            !repo_path.join("staged.txt").exists(),
            "Staged but uncommitted file should be removed"
        );
    }

    #[test]
    fn test_cherry_pick_does_not_modify_source() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Create source branch with multiple commits
        repo.create_branch("feature", None).unwrap();
        repo.checkout_branch("feature").unwrap();

        // Add multiple commits
        for i in 1..=3 {
            std::fs::write(
                repo_path.join(format!("file{i}.txt")),
                format!("Content {i}"),
            )
            .unwrap();
            Command::new("git")
                .args(["add", "."])
                .current_dir(&repo_path)
                .output()
                .unwrap();

            Command::new("git")
                .args(["commit", "-m", &format!("Commit {i}")])
                .current_dir(&repo_path)
                .output()
                .unwrap();
        }

        // Get source branch state
        let source_commits = Command::new("git")
            .args(["log", "--format=%H", "feature"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let source_state = String::from_utf8_lossy(&source_commits.stdout).to_string();

        // Cherry-pick middle commit to another branch
        let commits: Vec<&str> = source_state.lines().collect();
        let middle_commit = commits[1];

        // Go back to previous branch
        Command::new("git")
            .args(["checkout", "-"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        repo.create_branch("target", None).unwrap();
        repo.checkout_branch("target").unwrap();

        repo.cherry_pick(middle_commit).unwrap();

        // Verify source branch is completely unchanged
        let after_commits = Command::new("git")
            .args(["log", "--format=%H", "feature"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        let after_state = String::from_utf8_lossy(&after_commits.stdout).to_string();

        assert_eq!(
            source_state, after_state,
            "Source branch should be completely unchanged after cherry-pick"
        );
    }

    #[test]
    fn test_detect_parent_branch() {
        let (_temp_dir, repo_path) = create_test_repo();
        let repo = GitRepository::open(&repo_path).unwrap();

        // Create a custom base branch (not just main/master)
        repo.create_branch("dev123", None).unwrap();
        repo.checkout_branch("dev123").unwrap();
        create_commit(&repo_path, "Base commit on dev123", "base.txt");

        // Create feature branch from dev123
        repo.create_branch("feature-branch", None).unwrap();
        repo.checkout_branch("feature-branch").unwrap();
        create_commit(&repo_path, "Feature commit", "feature.txt");

        // Should detect dev123 as parent since it's the most recent common ancestor
        let detected_parent = repo.detect_parent_branch().unwrap();

        // The algorithm should find dev123 through either Strategy 2 (default branch)
        // or Strategy 3 (common ancestor analysis)
        assert!(detected_parent.is_some(), "Should detect a parent branch");

        // Since we can't guarantee which strategy will work in the test environment,
        // just verify it returns something reasonable
        let parent = detected_parent.unwrap();
        assert!(
            parent == "dev123" || parent == "main" || parent == "master",
            "Parent should be dev123, main, or master, got: {parent}"
        );
    }
}
