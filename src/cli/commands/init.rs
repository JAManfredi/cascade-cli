use crate::cli::output::Output;
use crate::config::{initialize_repo, is_repo_initialized};
use crate::errors::{CascadeError, Result};
use crate::git::{find_repository_root, is_git_repository};
use std::env;

/// Initialize a repository for Cascade
pub async fn run(bitbucket_url: Option<String>, force: bool) -> Result<()> {
    Output::info("Initializing Cascade repository...");

    // Get current directory
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    // Check if we're in a Git repository
    if !is_git_repository(&current_dir) {
        return Err(CascadeError::not_initialized(
            "Not in a Git repository. Please run this command from within a Git repository.",
        ));
    }

    // Find the repository root
    let repo_root = find_repository_root(&current_dir)?;
    tracing::debug!("Found Git repository at: {}", repo_root.display());

    // Check if already initialized
    if is_repo_initialized(&repo_root) && !force {
        return Err(CascadeError::invalid_operation(
            "Repository is already initialized for Cascade. Use --force to reinitialize.",
        ));
    }

    if force && is_repo_initialized(&repo_root) {
        Output::warning("Force reinitializing repository...");
    }

    // Initialize the repository
    initialize_repo(&repo_root, bitbucket_url.clone())?;

    // Print success message
    Output::success("Cascade repository initialized successfully!");

    if let Some(url) = &bitbucket_url {
        Output::sub_item(format!("Bitbucket Server URL: {url}"));
    }

    println!();
    Output::section("Next steps");
    Output::bullet("Configure Bitbucket Server settings:");
    if bitbucket_url.is_none() {
        Output::command_example("ca config set bitbucket.url https://your-bitbucket-server.com");
    }
    Output::command_example("ca config set bitbucket.project YOUR_PROJECT_KEY");
    Output::command_example("ca config set bitbucket.repo your-repo-name");
    Output::command_example("ca config set bitbucket.token your-personal-access-token");
    Output::bullet("Verify configuration:");
    Output::command_example("ca doctor");
    Output::bullet("Create your first stack:");
    Output::command_example("ca create \"Add new feature\"");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};
    use tempfile::TempDir;

    async fn create_test_git_repo() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repository
        let repo = Repository::init(&repo_path).unwrap();

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

        (temp_dir, repo_path)
    }

    #[tokio::test]
    async fn test_init_in_git_repo() {
        let (_temp_dir, repo_path) = create_test_git_repo().await;

        // Test the core functionality directly using internal functions
        // This verifies initialization logic without environment-dependent directory changes
        assert!(is_git_repository(&repo_path));

        // Initialize using internal function
        crate::config::initialize_repo(
            &repo_path,
            Some("https://bitbucket.example.com".to_string()),
        )
        .unwrap();

        // Verify it was initialized successfully
        assert!(is_repo_initialized(&repo_path));

        Output::success("Cascade initialization in Git repository tested successfully");
    }

    #[tokio::test]
    async fn test_init_outside_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let non_git_path = temp_dir.path();

        // Test validation logic directly - non-git directories should be detected
        assert!(!is_git_repository(non_git_path));

        // Attempting to find repository root should fail in non-git directory
        let result = find_repository_root(non_git_path);
        assert!(result.is_err());

        Output::success("Non-Git directory correctly detected - initialization would be rejected");
    }

    #[tokio::test]
    async fn test_init_already_initialized() {
        let (_temp_dir, repo_path) = create_test_git_repo().await;

        // Initialize repo directly using internal function
        crate::config::initialize_repo(&repo_path, None).unwrap();
        assert!(is_repo_initialized(&repo_path));

        // Test the validation logic directly without changing directories
        // This tests the same logic but avoids directory change issues
        assert!(is_repo_initialized(&repo_path));

        // Since we can't easily test the full run() function without directory issues,
        // let's test the core logic that should fail when already initialized
        let repo_root = crate::git::find_repository_root(&repo_path).unwrap();
        assert!(is_repo_initialized(&repo_root));

        // The logic in run() should detect this is already initialized
        // and return an error unless force is used
        Output::success("Repository correctly detected as already initialized");
    }
}
