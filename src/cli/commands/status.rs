use crate::config::{get_repo_config_dir, is_repo_initialized, Settings};
use crate::errors::{CascadeError, Result};
use crate::git::{get_current_repository, GitRepository};
use std::env;

/// Show repository overview and all stacks status
pub async fn run() -> Result<()> {
    println!("📊 Repository Overview");
    println!("━━━━━━━━━━━━━━━━━━━━━━");

    // Get current directory and repository
    let _current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let git_repo = match get_current_repository() {
        Ok(repo) => repo,
        Err(_) => {
            println!("❌ Not in a Git repository");
            return Ok(());
        }
    };

    // Show Git repository information
    show_git_status(&git_repo)?;

    // Show Cascade initialization status
    show_cascade_status(&git_repo)?;

    Ok(())
}

fn show_git_status(git_repo: &GitRepository) -> Result<()> {
    println!("\n🔧 Git Repository:");

    let repo_info = git_repo.get_info()?;

    // Repository path
    println!("  Path: {}", repo_info.path.display());

    // Current branch
    if let Some(branch) = &repo_info.head_branch {
        println!("  Current branch: {branch}");
    } else {
        println!("  Current branch: (detached HEAD)");
    }

    // Current commit
    if let Some(commit) = &repo_info.head_commit {
        println!("  HEAD commit: {}", &commit[..12]);
    }

    // Working directory status
    if repo_info.is_dirty {
        println!("  Working directory: ⚠️  Has uncommitted changes");
    } else {
        println!("  Working directory: ✅ Clean");
    }

    // Untracked files
    if !repo_info.untracked_files.is_empty() {
        println!(
            "  Untracked files: {} files",
            repo_info.untracked_files.len()
        );
        if repo_info.untracked_files.len() <= 5 {
            for file in &repo_info.untracked_files {
                println!("    - {file}");
            }
        } else {
            for file in repo_info.untracked_files.iter().take(3) {
                println!("    - {file}");
            }
            println!("    ... and {} more", repo_info.untracked_files.len() - 3);
        }
    } else {
        println!("  Untracked files: None");
    }

    // Branches
    let branches = git_repo.list_branches()?;
    println!("  Local branches: {} total", branches.len());

    Ok(())
}

fn show_cascade_status(git_repo: &GitRepository) -> Result<()> {
    println!("\n🌊 Cascade Status:");

    let repo_path = git_repo.path();

    if !is_repo_initialized(repo_path) {
        println!("  Status: ❌ Not initialized");
        println!("  Run 'cc init' to initialize this repository for Cascade");
        return Ok(());
    }

    println!("  Status: ✅ Initialized");

    // Load and show configuration
    let config_dir = get_repo_config_dir(repo_path)?;
    let config_file = config_dir.join("config.json");
    let settings = Settings::load_from_file(&config_file)?;

    // Check Bitbucket configuration
    println!("\n📡 Bitbucket Configuration:");

    let mut config_complete = true;

    if !settings.bitbucket.url.is_empty() {
        println!("  Server URL: ✅ {}", settings.bitbucket.url);
    } else {
        println!("  Server URL: ❌ Not configured");
        config_complete = false;
    }

    if !settings.bitbucket.project.is_empty() {
        println!("  Project Key: ✅ {}", settings.bitbucket.project);
    } else {
        println!("  Project Key: ❌ Not configured");
        config_complete = false;
    }

    if !settings.bitbucket.repo.is_empty() {
        println!("  Repository: ✅ {}", settings.bitbucket.repo);
    } else {
        println!("  Repository: ❌ Not configured");
        config_complete = false;
    }

    if let Some(token) = &settings.bitbucket.token {
        if !token.is_empty() {
            println!("  Auth Token: ✅ Configured");
        } else {
            println!("  Auth Token: ❌ Not configured");
            config_complete = false;
        }
    } else {
        println!("  Auth Token: ❌ Not configured");
        config_complete = false;
    }

    // Configuration status summary
    println!("\n⚙️  Configuration:");
    if config_complete {
        println!("  Status: ✅ Ready for use");
    } else {
        println!("  Status: ⚠️  Incomplete configuration");
        println!("  Run 'cc config list' to see all settings");
        println!("  Run 'cc doctor' for configuration recommendations");
    }

    // Show stack information
    println!("\n📚 Stacks:");

    match crate::stack::StackManager::new(repo_path) {
        Ok(manager) => {
            let stacks = manager.get_all_stacks();
            let active_stack = manager.get_active_stack();

            if stacks.is_empty() {
                println!("  No stacks created yet");
                println!("  Run 'cc stacks create \"Stack Name\"' to create your first stack");
            } else {
                println!("  Total stacks: {}", stacks.len());

                // Show each stack with detailed status
                for stack in &stacks {
                    let is_active = active_stack
                        .as_ref()
                        .map(|a| a.name == stack.name)
                        .unwrap_or(false);
                    let active_marker = if is_active { "◉" } else { "◯" };

                    let submitted = stack.entries.iter().filter(|e| e.is_submitted).count();

                    let status_info = if submitted > 0 {
                        format!("{}/{} submitted", submitted, stack.entries.len())
                    } else if !stack.entries.is_empty() {
                        format!("{} entries, none submitted", stack.entries.len())
                    } else {
                        "empty".to_string()
                    };

                    println!("  {} {} - {}", active_marker, stack.name, status_info);

                    // Show additional details for active stack
                    if is_active && !stack.entries.is_empty() {
                        let first_branch = stack
                            .entries
                            .first()
                            .map(|e| e.branch.as_str())
                            .unwrap_or("unknown");
                        println!("    Base: {} → {}", stack.base_branch, first_branch);
                    }
                }

                if active_stack.is_none() && !stacks.is_empty() {
                    println!(
                        "\n  💡 No active stack. Use 'cc stacks switch <name>' to activate one"
                    );
                }
            }
        }
        Err(_) => {
            println!("  Unable to load stack information");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::initialize_repo;
    use git2::{Repository, Signature};
    use std::env;
    use tempfile::TempDir;

    async fn create_test_repo() -> (TempDir, std::path::PathBuf) {
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
    async fn test_status_uninitialized() {
        let (_temp_dir, repo_path) = create_test_repo().await;

        // Change to the repo directory (with proper error handling)
        let original_dir = env::current_dir().map_err(|_| "Failed to get current dir");
        match env::set_current_dir(&repo_path) {
            Ok(_) => {
                let result = run().await;

                // Restore original directory (best effort)
                if let Ok(orig) = original_dir {
                    let _ = env::set_current_dir(orig);
                }

                assert!(result.is_ok());
            }
            Err(_) => {
                // Skip test if we can't change directories (CI environment issue)
                println!("Skipping test due to directory access restrictions");
            }
        }
    }

    #[tokio::test]
    async fn test_status_initialized() {
        let (_temp_dir, repo_path) = create_test_repo().await;

        // Initialize Cascade
        initialize_repo(&repo_path, Some("https://test.bitbucket.com".to_string())).unwrap();

        // Change to the repo directory (with proper error handling)
        let original_dir = env::current_dir().map_err(|_| "Failed to get current dir");
        match env::set_current_dir(&repo_path) {
            Ok(_) => {
                let result = run().await;

                // Restore original directory (best effort)
                if let Ok(orig) = original_dir {
                    let _ = env::set_current_dir(orig);
                }

                assert!(result.is_ok());
            }
            Err(_) => {
                // Skip test if we can't change directories (CI environment issue)
                println!("Skipping test due to directory access restrictions");
            }
        }
    }
}
