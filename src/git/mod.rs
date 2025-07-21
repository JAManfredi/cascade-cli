pub mod branch_manager;
pub mod conflict_analysis;
pub mod repository;

pub use branch_manager::{BranchInfo, BranchManager};
pub use conflict_analysis::{ConflictAnalysis, ConflictAnalyzer, ConflictRegion, ConflictType};
pub use repository::{GitRepository, GitStatusSummary, RepositoryInfo};

use crate::errors::{CascadeError, Result};
use std::path::Path;

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
