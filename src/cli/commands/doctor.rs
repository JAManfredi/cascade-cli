use crate::cli::output::Output;
use crate::config::{get_repo_config_dir, is_repo_initialized, Settings};
use crate::errors::{CascadeError, Result};
use crate::git::{get_current_repository, is_git_repository};
use std::env;

/// Check repository health and configuration
pub async fn run() -> Result<()> {
    Output::section("Cascade Doctor");
    Output::info("Diagnosing repository health and configuration...");
    println!();

    let mut issues_found = 0;
    let mut warnings_found = 0;

    // Check 1: Git repository
    issues_found += check_git_repository().await?;

    // Check 2: Cascade initialization
    let (repo_issues, repo_warnings) = check_cascade_initialization().await?;
    issues_found += repo_issues;
    warnings_found += repo_warnings;

    // Check 3: Configuration
    if issues_found == 0 {
        let config_warnings = check_configuration().await?;
        warnings_found += config_warnings;
    }

    // Check 4: Git configuration
    warnings_found += check_git_configuration().await?;

    // Summary
    print_summary(issues_found, warnings_found);

    Ok(())
}

async fn check_git_repository() -> Result<u32> {
    Output::check_start("Checking Git repository");

    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    if !is_git_repository(&current_dir) {
        Output::error("Not in a Git repository");
        Output::solution("Navigate to a Git repository or run 'git init'");
        return Ok(1);
    }

    match get_current_repository() {
        Ok(git_repo) => {
            let repo_info = git_repo.get_info()?;
            Output::success(format!(
                "Git repository found at: {}",
                repo_info.path.display()
            ));

            if let Some(branch) = &repo_info.head_branch {
                Output::success(format!("Current branch: {branch}"));
            } else {
                Output::warning("Detached HEAD state");
            }
        }
        Err(e) => {
            Output::error(format!("Git repository error: {e}"));
            return Ok(1);
        }
    }

    Ok(0)
}

async fn check_cascade_initialization() -> Result<(u32, u32)> {
    Output::check_start("Checking Cascade initialization");

    let git_repo = get_current_repository()?;
    let repo_path = git_repo.path();

    if !is_repo_initialized(repo_path) {
        Output::error("Repository not initialized for Cascade");
        Output::solution("Run 'ca init' to initialize");
        return Ok((1, 0));
    }

    Output::success("Repository initialized for Cascade");

    // Check for configuration directory structure
    let config_dir = get_repo_config_dir(repo_path)?;

    if !config_dir.exists() {
        Output::error("Configuration directory missing");
        Output::solution("Run 'ca init --force' to recreate");
        return Ok((1, 0));
    }

    Output::success("Configuration directory exists");

    // Check for required subdirectories
    let stacks_dir = config_dir.join("stacks");
    let cache_dir = config_dir.join("cache");

    let mut warnings = 0;

    if !stacks_dir.exists() {
        Output::warning("Stacks directory missing");
        warnings += 1;
    } else {
        Output::success("Stacks directory exists");
    }

    if !cache_dir.exists() {
        Output::warning("Cache directory missing");
        warnings += 1;
    } else {
        Output::success("Cache directory exists");
    }

    Ok((0, warnings))
}

async fn check_configuration() -> Result<u32> {
    Output::check_start("Checking configuration");

    let git_repo = get_current_repository()?;
    let config_dir = get_repo_config_dir(git_repo.path())?;
    let config_file = config_dir.join("config.json");

    let settings = Settings::load_from_file(&config_file)?;
    let mut warnings = 0;

    // Validate configuration
    match settings.validate() {
        Ok(()) => {
            Output::success("Configuration is valid");
        }
        Err(e) => {
            Output::warning(format!("Configuration validation failed: {e}"));
            warnings += 1;
        }
    }

    // Check Bitbucket configuration completeness
    Output::check_start("Bitbucket configuration");

    if settings.bitbucket.url.is_empty() {
        Output::warning("Bitbucket server URL not configured");
        Output::solution("ca config set bitbucket.url https://your-bitbucket-server.com");
        warnings += 1;
    } else {
        Output::success("Bitbucket server URL configured");
    }

    if settings.bitbucket.project.is_empty() {
        Output::warning("Bitbucket project key not configured");
        Output::solution("ca config set bitbucket.project YOUR_PROJECT_KEY");
        warnings += 1;
    } else {
        Output::success("Bitbucket project key configured");
    }

    if settings.bitbucket.repo.is_empty() {
        Output::warning("Bitbucket repository slug not configured");
        Output::solution("ca config set bitbucket.repo your-repo-name");
        warnings += 1;
    } else {
        Output::success("Bitbucket repository slug configured");
    }

    if settings
        .bitbucket
        .token
        .as_ref()
        .is_none_or(|s| s.is_empty())
    {
        Output::warning("Bitbucket authentication token not configured");
        Output::solution("ca config set bitbucket.token your-personal-access-token");
        warnings += 1;
    } else {
        Output::success("Bitbucket authentication token configured");
    }

    Ok(warnings)
}

async fn check_git_configuration() -> Result<u32> {
    Output::check_start("Checking Git configuration");

    let git_repo = get_current_repository()?;
    let repo_path = git_repo.path();
    let git_repo_inner = git2::Repository::open(repo_path)?;

    let mut warnings = 0;

    // Check Git user configuration
    match git_repo_inner.config() {
        Ok(config) => {
            match config.get_string("user.name") {
                Ok(name) => {
                    Output::success(format!("Git user.name: {name}"));
                }
                Err(_) => {
                    Output::warning("Git user.name not configured");
                    Output::solution("git config user.name \"Your Name\"");
                    warnings += 1;
                }
            }

            match config.get_string("user.email") {
                Ok(email) => {
                    Output::success(format!("Git user.email: {email}"));
                }
                Err(_) => {
                    Output::warning("Git user.email not configured");
                    Output::solution("git config user.email \"your.email@example.com\"");
                    warnings += 1;
                }
            }
        }
        Err(_) => {
            Output::warning("Could not read Git configuration");
            warnings += 1;
        }
    }

    // Check for remote repositories
    match git_repo_inner.remotes() {
        Ok(remotes) => {
            if remotes.is_empty() {
                Output::warning("No remote repositories configured");
                Output::tip("Add a remote with 'git remote add origin <url>'");
                warnings += 1;
            } else {
                Output::success(format!("Remote repositories configured: {}", remotes.len()));
            }
        }
        Err(_) => {
            Output::warning("Could not read remote repositories");
            warnings += 1;
        }
    }

    Ok(warnings)
}

fn print_summary(issues: u32, warnings: u32) {
    Output::section("Summary");

    if issues == 0 && warnings == 0 {
        Output::success("All checks passed! Your repository is ready for Cascade.");
        println!();
        Output::tip("Next steps:");
        Output::bullet("Create your first stack: ca create \"Add new feature\"");
        Output::bullet("Submit for review: ca submit");
        Output::bullet("View help: ca --help");
    } else if issues == 0 {
        Output::warning(format!(
            "{} warning{} found, but no critical issues.",
            warnings,
            if warnings == 1 { "" } else { "s" }
        ));
        Output::sub_item(
            "Your repository should work, but consider addressing the warnings above.",
        );
    } else {
        Output::error(format!(
            "{} critical issue{} found that need to be resolved.",
            issues,
            if issues == 1 { "" } else { "s" }
        ));
        if warnings > 0 {
            Output::sub_item(format!(
                "Additionally, {} warning{} found.",
                warnings,
                if warnings == 1 { "" } else { "s" }
            ));
        }
        Output::sub_item("Please address the issues above before using Cascade.");
    }
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

        // Configure git user and default branch
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        config.set_str("init.defaultBranch", "main").unwrap();

        // Create initial commit
        let signature = Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();

        let commit_oid = repo
            .commit(
                None, // Create initial commit without updating HEAD
                &signature,
                &signature,
                "Initial commit",
                &tree,
                &[],
            )
            .unwrap();

        // Create main branch and set HEAD
        let commit = repo.find_commit(commit_oid).unwrap();
        repo.branch("main", &commit, false).unwrap();
        repo.set_head("refs/heads/main").unwrap();
        repo.checkout_head(None).unwrap();

        (temp_dir, repo_path)
    }

    #[tokio::test]
    async fn test_doctor_uninitialized() {
        let (_temp_dir, repo_path) = create_test_repo().await;

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&repo_path).unwrap();

        let result = run().await;

        // Restore directory (best effort - may fail if temp dir already cleaned up)
        let _ = env::set_current_dir(&original_dir);

        if let Err(e) = &result {
            eprintln!("Doctor command failed: {e}");
        }
        assert!(result.is_ok());

        // _temp_dir dropped here automatically
    }

    #[tokio::test]
    async fn test_doctor_initialized() {
        let (_temp_dir, repo_path) = create_test_repo().await;

        // Initialize Cascade
        initialize_repo(&repo_path, Some("https://test.bitbucket.com".to_string())).unwrap();

        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&repo_path).unwrap();

        let result = run().await;

        // Restore directory (best effort - may fail if temp dir already cleaned up)
        let _ = env::set_current_dir(&original_dir);

        if let Err(e) = &result {
            eprintln!("Doctor command failed: {e}");
        }
        assert!(result.is_ok());

        // _temp_dir dropped here automatically
    }
}
