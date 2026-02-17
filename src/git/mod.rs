pub mod branch_manager;
pub mod conflict_analysis;
pub mod repository;

pub use branch_manager::{BranchInfo, BranchManager};
pub use conflict_analysis::{ConflictAnalysis, ConflictAnalyzer, ConflictRegion, ConflictType};
pub use repository::{GitRepository, GitStatusSummary, RepositoryInfo};

use crate::errors::{CascadeError, Result};
use std::path::{Path, PathBuf};

/// Resolve the per-worktree git directory from a workdir path.
/// Handles both normal repos (.git is a directory) and worktrees (.git is a file
/// containing `gitdir: <path>`).
pub fn resolve_git_dir(workdir: &Path) -> Result<PathBuf> {
    let git_path = workdir.join(".git");
    if git_path.is_dir() {
        Ok(git_path)
    } else if git_path.is_file() {
        let content = std::fs::read_to_string(&git_path)
            .map_err(|e| CascadeError::config(format!("Failed to read .git file: {e}")))?;
        let gitdir = content
            .strip_prefix("gitdir: ")
            .map(|s| s.trim())
            .ok_or_else(|| CascadeError::config("Invalid .git file format"))?;
        let resolved = if std::path::Path::new(gitdir).is_absolute() {
            PathBuf::from(gitdir)
        } else {
            workdir.join(gitdir)
        };
        Ok(resolved)
    } else {
        Err(CascadeError::config(format!(
            "Not a git repository: {}",
            git_path.display()
        )))
    }
}

/// Check if a directory is a Git repository
pub fn is_git_repository(path: &Path) -> bool {
    path.join(".git").exists() || git2::Repository::discover(path).is_ok()
}

/// Find the root of the Git repository
pub fn find_repository_root(start_path: &Path) -> Result<std::path::PathBuf> {
    let repo = git2::Repository::discover(start_path).map_err(CascadeError::Git)?;

    let workdir = repo
        .workdir()
        .ok_or_else(|| CascadeError::config("Repository has no working directory (bare repo?)"))?;

    Ok(workdir.to_path_buf())
}

/// Get the current working directory as a Git repository
pub fn get_current_repository() -> Result<GitRepository> {
    let current_dir = std::env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)?;
    GitRepository::open(&repo_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_resolve_git_dir_normal_repo() {
        let tmp = TempDir::new().unwrap();
        let git_dir = tmp.path().join(".git");
        fs::create_dir(&git_dir).unwrap();

        let result = resolve_git_dir(tmp.path()).unwrap();
        assert_eq!(result, git_dir);
    }

    #[test]
    fn test_resolve_git_dir_worktree_absolute() {
        let tmp = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();
        let git_file = tmp.path().join(".git");
        fs::write(&git_file, format!("gitdir: {}", target.path().display())).unwrap();

        let result = resolve_git_dir(tmp.path()).unwrap();
        assert_eq!(result, target.path());
    }

    #[test]
    fn test_resolve_git_dir_worktree_relative() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("actual_git_dir");
        fs::create_dir(&target).unwrap();
        let git_file = tmp.path().join(".git");
        fs::write(&git_file, "gitdir: actual_git_dir").unwrap();

        let result = resolve_git_dir(tmp.path()).unwrap();
        assert_eq!(result, tmp.path().join("actual_git_dir"));
    }

    #[test]
    fn test_resolve_git_dir_worktree_with_trailing_newline() {
        let tmp = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();
        let git_file = tmp.path().join(".git");
        fs::write(&git_file, format!("gitdir: {}\n", target.path().display())).unwrap();

        let result = resolve_git_dir(tmp.path()).unwrap();
        assert_eq!(result, target.path());
    }

    #[test]
    fn test_resolve_git_dir_not_a_repo() {
        let tmp = TempDir::new().unwrap();
        let result = resolve_git_dir(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_git_dir_invalid_git_file() {
        let tmp = TempDir::new().unwrap();
        let git_file = tmp.path().join(".git");
        fs::write(&git_file, "not a valid git file").unwrap();

        let result = resolve_git_dir(tmp.path());
        assert!(result.is_err());
    }
}
