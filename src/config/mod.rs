pub mod auth;
pub mod settings;

pub use auth::{AuthConfig, AuthManager};
pub use settings::{BitbucketConfig, CascadeConfig, CascadeSettings, GitConfig, Settings};

use crate::errors::{CascadeError, Result};
use crate::git::GitRepository;
use std::fs;
use std::path::{Path, PathBuf};

/// Get the Cascade configuration directory (~/.cascade/)
pub fn get_config_dir() -> Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| CascadeError::config("Could not find home directory"))?;
    let config_dir = home_dir.join(".cascade");

    // Validate the path to ensure it's within the home directory
    crate::utils::path_validation::validate_config_path(&config_dir, &home_dir)
}

/// Get the Cascade configuration directory for a specific repository.
/// In a worktree, always uses the main repo's `.cascade/` directory
/// so that config, stacks, and credentials are shared across worktrees.
pub fn get_repo_config_dir(repo_path: &Path) -> Result<PathBuf> {
    // Validate that repo_path is a real directory
    let canonical_repo = repo_path.canonicalize().map_err(|e| {
        CascadeError::config(format!("Invalid repository path '{repo_path:?}': {e}"))
    })?;

    // Check if we're in a worktree by comparing commondir to the repo path.
    // commondir() points to the shared .git dir; canonicalize to strip any
    // trailing slash, then take parent() to get the main repo root.
    if let Ok(repo) = git2::Repository::discover(repo_path) {
        let commondir = repo.commondir().to_path_buf();
        let commondir_clean = commondir.canonicalize().unwrap_or(commondir);
        if let Some(main_root) = commondir_clean.parent() {
            let main_canonical = main_root.canonicalize().unwrap_or(main_root.to_path_buf());
            if main_canonical != canonical_repo {
                // We're in a worktree — always use the main repo's .cascade/
                let main_config_dir = main_canonical.join(".cascade");
                return crate::utils::path_validation::validate_config_path(
                    &main_config_dir,
                    &main_canonical,
                );
            }
        }
    }

    // Not a worktree — use the repo's own .cascade/
    let config_dir = canonical_repo.join(".cascade");
    crate::utils::path_validation::validate_config_path(&config_dir, &canonical_repo)
}

/// Ensure the configuration directory exists
pub fn ensure_config_dir(config_dir: &Path) -> Result<()> {
    if !config_dir.exists() {
        fs::create_dir_all(config_dir)
            .map_err(|e| CascadeError::config(format!("Failed to create config directory: {e}")))?;
    }

    // Create subdirectories
    let stacks_dir = config_dir.join("stacks");
    if !stacks_dir.exists() {
        fs::create_dir_all(&stacks_dir)
            .map_err(|e| CascadeError::config(format!("Failed to create stacks directory: {e}")))?;
    }

    let cache_dir = config_dir.join("cache");
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir)
            .map_err(|e| CascadeError::config(format!("Failed to create cache directory: {e}")))?;
    }

    Ok(())
}

/// Check if a repository is initialized for Cascade
pub fn is_repo_initialized(repo_path: &Path) -> bool {
    match get_repo_config_dir(repo_path) {
        Ok(config_dir) => config_dir.exists() && config_dir.join("config.json").exists(),
        Err(_) => false,
    }
}

/// Initialize a repository for Cascade
pub fn initialize_repo(repo_path: &Path, bitbucket_url: Option<String>) -> Result<()> {
    let config_dir = get_repo_config_dir(repo_path)?;
    ensure_config_dir(&config_dir)?;

    // Create default configuration with detected default branch
    let mut settings = Settings::default_for_repo(bitbucket_url);

    // Detect the actual default branch from the repository
    if let Ok(git_repo) = GitRepository::open(repo_path) {
        if let Ok(detected_branch) = git_repo.detect_main_branch() {
            tracing::debug!("Detected default branch: {}", detected_branch);
            settings.git.default_branch = detected_branch;
        } else {
            tracing::debug!(
                "Could not detect default branch, using fallback: {}",
                settings.git.default_branch
            );
        }
    } else {
        tracing::debug!(
            "Could not open git repository, using fallback default branch: {}",
            settings.git.default_branch
        );
    }

    settings.save_to_file(&config_dir.join("config.json"))?;

    tracing::debug!("Initialized Cascade repository at {}", repo_path.display());
    Ok(())
}
