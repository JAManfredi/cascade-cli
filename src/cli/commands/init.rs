use crate::errors::{CascadeError, Result};
use crate::config::{initialize_repo, is_repo_initialized};
use crate::git::{is_git_repository, find_repository_root};
use std::env;

/// Initialize a repository for Cascade
pub async fn run(bitbucket_url: Option<String>, force: bool) -> Result<()> {
    tracing::info!("Initializing Cascade repository...");
    
    // Get current directory
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;
    
    // Check if we're in a Git repository
    if !is_git_repository(&current_dir) {
        return Err(CascadeError::not_initialized(
            "Not in a Git repository. Please run this command from within a Git repository."
        ));
    }
    
    // Find the repository root
    let repo_root = find_repository_root(&current_dir)?;
    tracing::debug!("Found Git repository at: {}", repo_root.display());
    
    // Check if already initialized
    if is_repo_initialized(&repo_root) && !force {
        return Err(CascadeError::invalid_operation(
            "Repository is already initialized for Cascade. Use --force to reinitialize."
        ));
    }
    
    if force && is_repo_initialized(&repo_root) {
        tracing::warn!("Force reinitializing repository...");
    }
    
    // Initialize the repository
    initialize_repo(&repo_root, bitbucket_url.clone())?;
    
    // Print success message
    println!("✅ Cascade repository initialized successfully!");
    
    if let Some(url) = &bitbucket_url {
        println!("📊 Bitbucket Server URL: {}", url);
    }
    
    println!("\n📋 Next steps:");
    println!("  1. Configure Bitbucket Server settings:");
    if bitbucket_url.is_none() {
        println!("     cc config set bitbucket.url https://your-bitbucket-server.com");
    }
    println!("     cc config set bitbucket.project YOUR_PROJECT_KEY");
    println!("     cc config set bitbucket.repo your-repo-name");
    println!("     cc config set bitbucket.token your-personal-access-token");
    println!("  2. Verify configuration:");
    println!("     cc doctor");
    println!("  3. Create your first stack:");
    println!("     cc create \"Add new feature\"");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use git2::{Repository, Signature};

    
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
        ).unwrap();
        
        (temp_dir, repo_path)
    }
    
    #[tokio::test]
    async fn test_init_in_git_repo() {
        let (_temp_dir, repo_path) = create_test_git_repo().await;
        
        // Change to the repo directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&repo_path).unwrap();
        
        // Initialize Cascade
        let result = run(Some("https://bitbucket.example.com".to_string()), false).await;
        
        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
        
        assert!(result.is_ok());
        assert!(is_repo_initialized(&repo_path));
    }
    
    #[tokio::test]
    async fn test_init_outside_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let non_git_path = temp_dir.path();
        
        // Change to non-git directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(non_git_path).unwrap();
        
        // Try to initialize Cascade
        let result = run(None, false).await;
        
        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CascadeError::Config(_)));
    }
    
    #[tokio::test]
    async fn test_init_already_initialized() {
        let (_temp_dir, repo_path) = create_test_git_repo().await;
        
        // Change to the repo directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&repo_path).unwrap();
        
        // Initialize Cascade first time
        run(None, false).await.unwrap();
        
        // Try to initialize again without force
        let result = run(None, false).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CascadeError::Validation(_)));
        
        // Initialize with force should succeed
        let result = run(None, true).await;
        assert!(result.is_ok());
        
        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }
} 