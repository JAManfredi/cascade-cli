use crate::cli::output::Output;
use crate::errors::{CascadeError, Result};
use crate::git::GitRepository;
use chrono::{DateTime, Utc};
use std::env;

/// Run the cleanup command to remove orphaned temporary branches
pub async fn run(execute: bool, force: bool) -> Result<()> {
    let repo_path = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Failed to get current directory: {e}")))?;

    let git_repo = GitRepository::open(&repo_path)?;

    Output::section("🧹 Scanning for orphaned temporary branches");

    // Find all branches matching temp pattern: *-temp-*
    let all_branches = git_repo.list_branches()?;
    let temp_branches: Vec<String> = all_branches
        .iter()
        .filter(|b| b.contains("-temp-"))
        .cloned()
        .collect();

    if temp_branches.is_empty() {
        Output::success("✓ No orphaned temporary branches found");
        return Ok(());
    }

    Output::info(format!("Found {} temporary branches:", temp_branches.len()));

    // Analyze each temp branch
    for branch_name in &temp_branches {
        let branch_info = analyze_temp_branch(&git_repo, branch_name)?;

        Output::sub_item(format!("  {} {}", branch_name, branch_info));
    }

    println!(); // Blank line

    if !execute {
        Output::warning("🔍 DRY RUN MODE - No branches will be deleted");
        Output::info("Run with --execute to actually delete these branches");
        Output::info("Use --force to delete branches with unmerged commits");
        return Ok(());
    }

    // Actually delete the branches
    Output::section(format!(
        "Deleting {} temporary branches...",
        temp_branches.len()
    ));

    let mut deleted = 0;
    let mut failed = 0;

    for branch_name in &temp_branches {
        match git_repo.delete_branch_unsafe(branch_name) {
            Ok(_) => {
                Output::success(format!("✓ Deleted: {}", branch_name));
                deleted += 1;
            }
            Err(e) if !force => {
                Output::warning(format!("⚠️  Skipped: {} ({})", branch_name, e));
                Output::sub_item("   Use --force to delete branches with unmerged commits");
                failed += 1;
            }
            Err(e) => {
                Output::error(format!("✗ Failed to delete: {} ({})", branch_name, e));
                failed += 1;
            }
        }
    }

    println!(); // Blank line

    if deleted > 0 {
        Output::success(format!("✓ Successfully deleted {} branches", deleted));
    }

    if failed > 0 {
        Output::warning(format!("⚠️  {} branches could not be deleted", failed));
    }

    Ok(())
}

/// Analyze a temporary branch and return info about it
fn analyze_temp_branch(git_repo: &GitRepository, branch_name: &str) -> Result<String> {
    // Try to extract timestamp from branch name (format: *-temp-1234567890)
    let parts: Vec<&str> = branch_name.split("-temp-").collect();

    if parts.len() == 2 {
        if let Ok(timestamp) = parts[1].parse::<i64>() {
            if let Some(created_at) = DateTime::from_timestamp(timestamp, 0) {
                let now = Utc::now();
                let age = now.signed_duration_since(created_at);

                if age.num_days() > 0 {
                    return Ok(format!("(created {} days ago)", age.num_days()));
                } else if age.num_hours() > 0 {
                    return Ok(format!("(created {} hours ago)", age.num_hours()));
                } else {
                    return Ok(format!("(created {} minutes ago)", age.num_minutes()));
                }
            }
        }
    }

    // Try to get last commit info
    match git_repo.get_branch_commit_hash(branch_name) {
        Ok(commit_hash) => Ok(format!("(commit: {})", &commit_hash[..8])),
        Err(_) => Ok("(orphaned)".to_string()),
    }
}
