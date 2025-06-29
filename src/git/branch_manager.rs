use crate::errors::Result;
use crate::git::GitRepository;
use serde::{Deserialize, Serialize};

/// Information about a branch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub commit_hash: String,
    pub is_current: bool,
    pub upstream: Option<String>,
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

            branch_info.push(BranchInfo {
                name: branch_name,
                commit_hash,
                is_current,
                upstream: None, // TODO: Implement upstream tracking
            });
        }

        Ok(branch_info)
    }

    /// Get the commit hash for a specific branch safely without switching branches
    fn get_branch_commit_hash(&self, branch_name: &str) -> Result<String> {
        self.git_repo.get_branch_commit_hash(branch_name)
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
    }
}
