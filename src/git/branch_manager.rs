use crate::errors::Result;
use crate::git::GitRepository;
use serde::{Deserialize, Serialize};

/// Information about upstream tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamInfo {
    pub remote: String,
    pub branch: String,
    pub full_name: String, // e.g., "origin/feature-auth"
    pub ahead: usize,      // commits ahead of upstream
    pub behind: usize,     // commits behind upstream
}

/// Information about a branch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub commit_hash: String,
    pub is_current: bool,
    pub upstream: Option<UpstreamInfo>,
}

/// Manages branch operations and metadata
pub struct BranchManager {
    git_repo: GitRepository,
}

impl BranchManager {
    /// Create a new BranchManager
    pub fn new(git_repo: GitRepository) -> Self {
        Self { git_repo }
    }

    /// Get information about all branches
    pub fn get_branch_info(&self) -> Result<Vec<BranchInfo>> {
        let branches = self.git_repo.list_branches()?;
        let current_branch = self.git_repo.get_current_branch().ok();

        let mut branch_info = Vec::new();
        for branch_name in branches {
            let commit_hash = self.get_branch_commit_hash(&branch_name)?;
            let is_current = current_branch.as_ref() == Some(&branch_name);
            let upstream = self.get_upstream_info(&branch_name)?;

            branch_info.push(BranchInfo {
                name: branch_name,
                commit_hash,
                is_current,
                upstream,
            });
        }

        Ok(branch_info)
    }

    /// Get the commit hash for a specific branch safely without switching branches
    fn get_branch_commit_hash(&self, branch_name: &str) -> Result<String> {
        self.git_repo.get_branch_commit_hash(branch_name)
    }

    /// Get upstream tracking information for a branch
    fn get_upstream_info(&self, branch_name: &str) -> Result<Option<UpstreamInfo>> {
        // First, try to get the upstream tracking from git config
        if let Some(upstream) = self.git_repo.get_upstream_branch(branch_name)? {
            let (remote, remote_branch) = self.parse_upstream_name(&upstream)?;

            // Calculate ahead/behind counts
            let (ahead, behind) = self.calculate_ahead_behind_counts(branch_name, &upstream)?;

            Ok(Some(UpstreamInfo {
                remote,
                branch: remote_branch,
                full_name: upstream,
                ahead,
                behind,
            }))
        } else {
            // No upstream configured
            Ok(None)
        }
    }

    /// Parse upstream name like "origin/feature-auth" into remote and branch
    fn parse_upstream_name(&self, upstream: &str) -> Result<(String, String)> {
        let parts: Vec<&str> = upstream.splitn(2, '/').collect();
        if parts.len() == 2 {
            Ok((parts[0].to_string(), parts[1].to_string()))
        } else {
            // Fallback - assume origin if no slash
            Ok(("origin".to_string(), upstream.to_string()))
        }
    }

    /// Calculate how many commits ahead/behind the local branch is from upstream
    fn calculate_ahead_behind_counts(
        &self,
        local_branch: &str,
        upstream_branch: &str,
    ) -> Result<(usize, usize)> {
        match self
            .git_repo
            .get_ahead_behind_counts(local_branch, upstream_branch)
        {
            Ok((ahead, behind)) => Ok((ahead, behind)),
            Err(_) => {
                // If we can't calculate (e.g., remote doesn't exist), return 0,0
                Ok((0, 0))
            }
        }
    }

    /// Generate a safe branch name from a commit message
    pub fn generate_branch_name(&self, message: &str) -> String {
        let base_name = message
            .to_lowercase()
            .chars()
            .map(|c| match c {
                'a'..='z' | '0'..='9' => c,
                _ => '-',
            })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .take(5) // Limit to first 5 words
            .collect::<Vec<_>>()
            .join("-");

        // Ensure the branch name is unique
        let mut counter = 1;
        let mut candidate = base_name.clone();

        while self.git_repo.branch_exists(&candidate) {
            candidate = format!("{base_name}-{counter}");
            counter += 1;
        }

        // Ensure it starts with a letter
        if candidate.chars().next().is_none_or(|c| !c.is_alphabetic()) {
            candidate = format!("feature-{candidate}");
        }

        candidate
    }

    /// Create a new branch with a generated name
    pub fn create_branch_from_message(
        &self,
        message: &str,
        target: Option<&str>,
    ) -> Result<String> {
        let branch_name = self.generate_branch_name(message);
        self.git_repo.create_branch(&branch_name, target)?;
        Ok(branch_name)
    }

    /// Set upstream tracking for a branch
    pub fn set_upstream(&self, branch_name: &str, remote: &str, remote_branch: &str) -> Result<()> {
        self.git_repo
            .set_upstream(branch_name, remote, remote_branch)
    }

    /// Get upstream info for a specific branch
    pub fn get_branch_upstream(&self, branch_name: &str) -> Result<Option<UpstreamInfo>> {
        self.get_upstream_info(branch_name)
    }

    /// Check if a branch has upstream tracking
    pub fn has_upstream(&self, branch_name: &str) -> Result<bool> {
        Ok(self.get_upstream_info(branch_name)?.is_some())
    }

    /// Get the underlying Git repository
    pub fn git_repo(&self) -> &GitRepository {
        &self.git_repo
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::repository::*;
    use git2::{Repository, Signature};
    use tempfile::TempDir;

    fn create_test_branch_manager() -> (TempDir, BranchManager) {
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
        )
        .unwrap();

        let git_repo = GitRepository::open(repo_path).unwrap();
        let branch_manager = BranchManager::new(git_repo);

        (temp_dir, branch_manager)
    }

    #[test]
    fn test_branch_name_generation() {
        let (_temp_dir, branch_manager) = create_test_branch_manager();

        assert_eq!(
            branch_manager.generate_branch_name("Add user authentication"),
            "add-user-authentication"
        );

        assert_eq!(
            branch_manager.generate_branch_name("Fix bug in payment system!!!"),
            "fix-bug-in-payment-system"
        );

        assert_eq!(
            branch_manager.generate_branch_name("123 numeric start"),
            "feature-123-numeric-start"
        );
    }

    #[test]
    fn test_branch_creation() {
        let (_temp_dir, branch_manager) = create_test_branch_manager();

        let branch_name = branch_manager
            .create_branch_from_message("Add login feature", None)
            .unwrap();

        assert_eq!(branch_name, "add-login-feature");
        assert!(branch_manager.git_repo().branch_exists(&branch_name));
    }

    #[test]
    fn test_branch_info() {
        let (_temp_dir, branch_manager) = create_test_branch_manager();

        // Create a test branch
        let _branch_name = branch_manager
            .create_branch_from_message("Test feature", None)
            .unwrap();

        let branch_info = branch_manager.get_branch_info().unwrap();
        assert!(!branch_info.is_empty());

        // Should have at least one branch (the default branch, whether it's "main" or "master")
        // and at least one should be marked as current
        assert!(branch_info.iter().any(|b| b.is_current));

        // Should have at least 2 branches (default + the test feature branch we created)
        assert!(branch_info.len() >= 2);

        // Test that upstream info is included (even if None for test branches)
        for branch in &branch_info {
            // In a test environment, branches typically don't have upstream
            // but the field should exist and be None
            assert!(branch.upstream.is_none());
        }
    }

    #[test]
    fn test_upstream_parsing() {
        let (_temp_dir, branch_manager) = create_test_branch_manager();

        // Test parsing upstream names
        let (remote, branch) = branch_manager
            .parse_upstream_name("origin/feature-auth")
            .unwrap();
        assert_eq!(remote, "origin");
        assert_eq!(branch, "feature-auth");

        let (remote, branch) = branch_manager.parse_upstream_name("upstream/main").unwrap();
        assert_eq!(remote, "upstream");
        assert_eq!(branch, "main");

        // Test fallback for names without slash
        let (remote, branch) = branch_manager.parse_upstream_name("feature-auth").unwrap();
        assert_eq!(remote, "origin");
        assert_eq!(branch, "feature-auth");
    }
}
