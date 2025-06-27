use crate::errors::{CascadeError, Result};
use git2::{Repository, Signature, Oid};
use std::path::{Path, PathBuf};

/// Repository information
#[derive(Debug, Clone)]
pub struct RepositoryInfo {
    pub path: PathBuf,
    pub head_branch: Option<String>,
    pub head_commit: Option<String>,
    pub is_dirty: bool,
    pub untracked_files: Vec<String>,
}

/// Wrapper around git2::Repository with safe operations
pub struct GitRepository {
    repo: Repository,
    path: PathBuf,
}

impl GitRepository {
    /// Open a Git repository at the given path
    pub fn open(path: &Path) -> Result<Self> {
        let repo = Repository::discover(path)
            .map_err(|e| CascadeError::config(format!("Not a git repository: {}", e)))?;
        
        let workdir = repo.workdir()
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
        let head = self.repo.head()
            .map_err(|e| CascadeError::branch(format!("Could not get HEAD: {}", e)))?;
        
        if let Some(name) = head.shorthand() {
            Ok(name.to_string())
        } else {
            // Detached HEAD - return commit hash
            let commit = head.peel_to_commit()
                .map_err(|e| CascadeError::branch(format!("Could not get HEAD commit: {}", e)))?;
            Ok(format!("HEAD@{}", commit.id()))
        }
    }
    
    /// Get the HEAD commit hash
    pub fn get_head_commit_hash(&self) -> Result<String> {
        let head = self.repo.head()
            .map_err(|e| CascadeError::branch(format!("Could not get HEAD: {}", e)))?;
        
        let commit = head.peel_to_commit()
            .map_err(|e| CascadeError::branch(format!("Could not get HEAD commit: {}", e)))?;
        
        Ok(commit.id().to_string())
    }
    
    /// Check if the working directory is dirty (has uncommitted changes)
    pub fn is_dirty(&self) -> Result<bool> {
        let statuses = self.repo.statuses(None)
            .map_err(|e| CascadeError::Git(e))?;
        
        for status in statuses.iter() {
            let flags = status.status();
            
            // Check for any modifications, additions, or deletions
            if flags.intersects(
                git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_NEW
                | git2::Status::INDEX_DELETED
                | git2::Status::WT_MODIFIED
                | git2::Status::WT_NEW
                | git2::Status::WT_DELETED
            ) {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// Get list of untracked files
    pub fn get_untracked_files(&self) -> Result<Vec<String>> {
        let statuses = self.repo.statuses(None)
            .map_err(|e| CascadeError::Git(e))?;
        
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
            let target_obj = self.repo.revparse_single(target)
                .map_err(|e| CascadeError::branch(format!("Could not find target '{}': {}", target, e)))?;
            target_obj.peel_to_commit()
                .map_err(|e| CascadeError::branch(format!("Target '{}' is not a commit: {}", target, e)))?
        } else {
            // Use current HEAD
            let head = self.repo.head()
                .map_err(|e| CascadeError::branch(format!("Could not get HEAD: {}", e)))?;
            head.peel_to_commit()
                .map_err(|e| CascadeError::branch(format!("Could not get HEAD commit: {}", e)))?
        };
        
        self.repo.branch(name, &target_commit, false)
            .map_err(|e| CascadeError::branch(format!("Could not create branch '{}': {}", name, e)))?;
        
        tracing::info!("Created branch '{}'", name);
        Ok(())
    }
    
    /// Switch to a branch
    pub fn checkout_branch(&self, name: &str) -> Result<()> {
        // Find the branch
        let branch = self.repo.find_branch(name, git2::BranchType::Local)
            .map_err(|e| CascadeError::branch(format!("Could not find branch '{}': {}", name, e)))?;
        
        let branch_ref = branch.get();
        let tree = branch_ref.peel_to_tree()
            .map_err(|e| CascadeError::branch(format!("Could not get tree for branch '{}': {}", name, e)))?;
        
        // Checkout the tree
        self.repo.checkout_tree(tree.as_object(), None)
            .map_err(|e| CascadeError::branch(format!("Could not checkout branch '{}': {}", name, e)))?;
        
        // Update HEAD
        self.repo.set_head(&format!("refs/heads/{}", name))
            .map_err(|e| CascadeError::branch(format!("Could not update HEAD to '{}': {}", name, e)))?;
        
        tracing::info!("Switched to branch '{}'", name);
        Ok(())
    }
    
    /// Check if a branch exists
    pub fn branch_exists(&self, name: &str) -> bool {
        self.repo.find_branch(name, git2::BranchType::Local).is_ok()
    }
    
    /// List all local branches
    pub fn list_branches(&self) -> Result<Vec<String>> {
        let branches = self.repo.branches(Some(git2::BranchType::Local))
            .map_err(|e| CascadeError::Git(e))?;
        
        let mut branch_names = Vec::new();
        for branch in branches {
            let (branch, _) = branch.map_err(|e| CascadeError::Git(e))?;
            if let Some(name) = branch.name().map_err(|e| CascadeError::Git(e))? {
                branch_names.push(name.to_string());
            }
        }
        
        Ok(branch_names)
    }
    
    /// Create a commit with all staged changes
    pub fn commit(&self, message: &str) -> Result<String> {
        let signature = self.get_signature()?;
        let tree_id = self.get_index_tree()?;
        let tree = self.repo.find_tree(tree_id)
            .map_err(|e| CascadeError::Git(e))?;
        
        // Get parent commits
        let head = self.repo.head()
            .map_err(|e| CascadeError::Git(e))?;
        let parent_commit = head.peel_to_commit()
            .map_err(|e| CascadeError::Git(e))?;
        
        let commit_id = self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &[&parent_commit],
        ).map_err(|e| CascadeError::Git(e))?;
        
        tracing::info!("Created commit: {} - {}", commit_id, message);
        Ok(commit_id.to_string())
    }
    
    /// Stage all changes
    pub fn stage_all(&self) -> Result<()> {
        let mut index = self.repo.index()
            .map_err(|e| CascadeError::Git(e))?;
        
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| CascadeError::Git(e))?;
        
        index.write()
            .map_err(|e| CascadeError::Git(e))?;
        
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
    pub fn get_head_commit(&self) -> Result<git2::Commit> {
        let head = self.repo.head()
            .map_err(|e| CascadeError::branch(format!("Could not get HEAD: {}", e)))?;
        
        head.peel_to_commit()
            .map_err(|e| CascadeError::branch(format!("Could not get HEAD commit: {}", e)))
    }

    /// Get a commit object by hash
    pub fn get_commit(&self, commit_hash: &str) -> Result<git2::Commit> {
        let oid = Oid::from_str(commit_hash)
            .map_err(|e| CascadeError::Git(e))?;
        
        self.repo.find_commit(oid)
            .map_err(|e| CascadeError::Git(e))
    }

    /// Get the commit hash at the head of a branch
    pub fn get_branch_head(&self, branch_name: &str) -> Result<String> {
        let branch = self.repo.find_branch(branch_name, git2::BranchType::Local)
            .map_err(|e| CascadeError::branch(format!("Could not find branch '{}': {}", branch_name, e)))?;
        
        let commit = branch.get().peel_to_commit()
            .map_err(|e| CascadeError::branch(format!("Could not get commit for branch '{}': {}", branch_name, e)))?;
        
        Ok(commit.id().to_string())
    }
    
    /// Get a signature for commits
    fn get_signature(&self) -> Result<Signature> {
        // Try to get signature from Git config
        if let Ok(config) = self.repo.config() {
            if let (Ok(name), Ok(email)) = (config.get_string("user.name"), config.get_string("user.email")) {
                return Signature::now(&name, &email)
                    .map_err(|e| CascadeError::Git(e));
            }
        }
        
        // Fallback to default signature
        Signature::now("Cascade CLI", "cascade@example.com")
            .map_err(|e| CascadeError::Git(e))
    }
    
    /// Get the tree ID from the current index
    fn get_index_tree(&self) -> Result<Oid> {
        let mut index = self.repo.index()
            .map_err(|e| CascadeError::Git(e))?;
        
        index.write_tree()
            .map_err(|e| CascadeError::Git(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    
    fn create_test_repo() -> (TempDir, GitRepository) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        
        // Initialize repository
        let repo = Repository::init(repo_path).unwrap();
        
        // Create initial commit
        let signature = Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        ).unwrap();
        
        let git_repo = GitRepository::open(repo_path).unwrap();
        (temp_dir, git_repo)
    }
    
    #[test]
    fn test_repository_info() {
        let (_temp_dir, git_repo) = create_test_repo();
        let info = git_repo.get_info().unwrap();
        
        assert_eq!(info.head_branch, Some("master".to_string()));
        assert!(!info.is_dirty);
        assert!(info.untracked_files.is_empty());
    }
    
    #[test]
    fn test_branch_operations() {
        let (_temp_dir, git_repo) = create_test_repo();
        
        // Create new branch
        git_repo.create_branch("feature", None).unwrap();
        assert!(git_repo.branch_exists("feature"));
        
        // Switch to branch
        git_repo.checkout_branch("feature").unwrap();
        assert_eq!(git_repo.get_current_branch().unwrap(), "feature");
        
        // List branches
        let branches = git_repo.list_branches().unwrap();
        assert!(branches.contains(&"master".to_string()));
        assert!(branches.contains(&"feature".to_string()));
    }
} 