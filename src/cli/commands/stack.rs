use crate::errors::{CascadeError, Result};
use crate::stack::{StackManager, StackStatus};
use crate::git::GitRepository;
use clap::Subcommand;
use std::env;
use tracing::{info, warn};

#[derive(Subcommand)]
pub enum StackAction {
    /// Create a new stack
    Create {
        /// Name of the stack
        name: String,
        /// Base branch for the stack
        #[arg(long, short)]
        base: Option<String>,
        /// Description of the stack
        #[arg(long, short)]
        description: Option<String>,
    },
    
    /// List all stacks
    List {
        /// Show detailed information
        #[arg(long, short)]
        verbose: bool,
    },
    
    /// Switch to a different stack
    Switch {
        /// Name of the stack to switch to
        name: String,
    },
    
    /// Show detailed information about a stack
    Show {
        /// Name of the stack (defaults to active stack)
        name: Option<String>,
    },
    
    /// Push current commit to the top of the stack
    Push {
        /// Branch name for this commit
        #[arg(long, short)]
        branch: Option<String>,
        /// Commit message (if creating a new commit)
        #[arg(long, short)]
        message: Option<String>,
        /// Use specific commit hash instead of HEAD
        #[arg(long)]
        commit: Option<String>,
    },
    
    /// Pop the top commit from the stack
    Pop {
        /// Keep the branch (don't delete it)
        #[arg(long)]
        keep_branch: bool,
    },
    
    /// Submit a stack entry for review
    Submit {
        /// Stack entry number (1-based, defaults to top)
        entry: Option<usize>,
        /// Pull request title
        #[arg(long, short)]
        title: Option<String>,
        /// Pull request description
        #[arg(long, short)]
        description: Option<String>,
    },
    
    /// Sync stack with remote repository
    Sync {
        /// Force sync even if there are conflicts
        #[arg(long)]
        force: bool,
    },
    
    /// Rebase stack on updated base branch
    Rebase {
        /// Interactive rebase
        #[arg(long, short)]
        interactive: bool,
    },
    
    /// Delete a stack
    Delete {
        /// Name of the stack to delete
        name: String,
        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },
}

pub async fn run(action: StackAction) -> Result<()> {
    let _current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    match action {
        StackAction::Create { name, base, description } => {
            create_stack(name, base, description).await
        }
        StackAction::List { verbose } => {
            list_stacks(verbose).await
        }
        StackAction::Switch { name } => {
            switch_stack(name).await
        }
        StackAction::Show { name } => {
            show_stack(name).await
        }
        StackAction::Push { branch, message, commit } => {
            push_to_stack(branch, message, commit).await
        }
        StackAction::Pop { keep_branch } => {
            pop_from_stack(keep_branch).await
        }
        StackAction::Submit { entry, title, description } => {
            submit_entry(entry, title, description).await
        }
        StackAction::Sync { force } => {
            sync_stack(force).await
        }
        StackAction::Rebase { interactive } => {
            rebase_stack(interactive).await
        }
        StackAction::Delete { name, force } => {
            delete_stack(name, force).await
        }
    }
}

async fn create_stack(name: String, base: Option<String>, description: Option<String>) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let mut manager = StackManager::new(&current_dir)?;
    let stack_id = manager.create_stack(name.clone(), base.clone(), description.clone())?;

    info!("✅ Created stack '{}'", name);
    if let Some(base_branch) = base {
        info!("   Base branch: {}", base_branch);
    }
    if let Some(desc) = description {
        info!("   Description: {}", desc);
    }
    info!("   Stack ID: {}", stack_id);
    info!("   Stack is now active");

    Ok(())
}

async fn list_stacks(verbose: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let manager = StackManager::new(&current_dir)?;
    let stacks = manager.list_stacks();

    if stacks.is_empty() {
        info!("No stacks found. Create one with: cc stack create <name>");
        return Ok(());
    }

    println!("📚 Stacks:");
    for (stack_id, name, status, entry_count, active_marker) in stacks {
        let status_icon = match status {
            StackStatus::Clean => "✅",
            StackStatus::Dirty => "🔄",
            StackStatus::OutOfSync => "⚠️",
            StackStatus::Conflicted => "❌",
            StackStatus::Rebasing => "🔀",
        };

        let active_indicator = if active_marker.is_some() { " (active)" } else { "" };
        
        if verbose {
            println!("  {} {} [{}]{}", status_icon, name, entry_count, active_indicator);
            println!("    ID: {}", stack_id);
            if let Some(stack_meta) = manager.get_stack_metadata(&stack_id) {
                println!("    Base: {}", stack_meta.base_branch);
                if let Some(desc) = &stack_meta.description {
                    println!("    Description: {}", desc);
                }
                println!("    Commits: {} total, {} submitted", 
                    stack_meta.total_commits, stack_meta.submitted_commits);
                if stack_meta.has_conflicts {
                    println!("    ⚠️  Has conflicts");
                }
            }
            println!();
        } else {
            println!("  {} {} [{}]{}", status_icon, name, entry_count, active_indicator);
        }
    }

    if !verbose {
        println!("\nUse --verbose for more details");
    }

    Ok(())
}

async fn switch_stack(name: String) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let mut manager = StackManager::new(&current_dir)?;
    
    // Verify stack exists
    if manager.get_stack_by_name(&name).is_none() {
        return Err(CascadeError::config(format!("Stack '{}' not found", name)));
    }

    manager.set_active_stack_by_name(&name)?;
    info!("✅ Switched to stack '{}'", name);

    Ok(())
}

async fn show_stack(name: Option<String>) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let manager = StackManager::new(&current_dir)?;
    
    let stack = if let Some(name) = name {
        manager.get_stack_by_name(&name)
            .ok_or_else(|| CascadeError::config(format!("Stack '{}' not found", name)))?
    } else {
        manager.get_active_stack()
            .ok_or_else(|| CascadeError::config("No active stack. Use 'cc stack list' to see available stacks"))?
    };

    let stack_meta = manager.get_stack_metadata(&stack.id).unwrap();

    println!("📋 Stack: {}", stack.name);
    println!("   ID: {}", stack.id);
    println!("   Base: {}", stack.base_branch);
    
    if let Some(description) = &stack.description {
        println!("   Description: {}", description);
    }

    println!("   Status: {:?}", stack.status);
    println!("   Active: {}", if stack.is_active { "Yes" } else { "No" });
    println!("   Created: {}", stack.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
    println!("   Updated: {}", stack.updated_at.format("%Y-%m-%d %H:%M:%S UTC"));

    println!("\n📊 Statistics:");
    println!("   Total commits: {}", stack_meta.total_commits);
    println!("   Submitted: {}", stack_meta.submitted_commits);
    println!("   Merged: {}", stack_meta.merged_commits);
    if stack_meta.total_commits > 0 {
        println!("   Progress: {:.1}%", stack_meta.completion_percentage());
    }

    if !stack.entries.is_empty() {
        println!("\n🔗 Entries:");
        for (i, entry) in stack.entries.iter().enumerate() {
            let status_icon = if entry.is_submitted {
                if entry.is_synced { "✅" } else { "📤" }
            } else {
                "📝"
            };
            
            println!("   {}. {} {} ({})", 
                i + 1, 
                status_icon, 
                entry.short_message(50), 
                entry.short_hash()
            );
            println!("      Branch: {}", entry.branch);
            if let Some(pr_id) = &entry.pull_request_id {
                println!("      PR: {}", pr_id);
            }
        }
    } else {
        println!("\n📝 No entries yet. Use 'cc stack push' to add commits.");
    }

    Ok(())
}

async fn push_to_stack(branch: Option<String>, message: Option<String>, commit: Option<String>) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let mut manager = StackManager::new(&current_dir)?;
    let repo = GitRepository::open(&current_dir)?;

    // Get commit hash (HEAD by default)
    let commit_hash = if let Some(hash) = commit {
        hash
    } else {
        repo.get_head_commit()?.id().to_string()
    };

    // Get commit message
    let commit_msg = if let Some(msg) = message {
        msg
    } else {
        let commit_obj = repo.get_commit(&commit_hash)?;
        commit_obj.message().unwrap_or("").to_string()
    };

    // Generate branch name if not provided
    let branch_name = if let Some(branch) = branch {
        branch
    } else {
        let branch_mgr = crate::git::BranchManager::new(repo);
        branch_mgr.generate_branch_name(&commit_msg)
    };

    let entry_id = manager.push_to_stack(branch_name.clone(), commit_hash.clone(), commit_msg.clone())?;

    info!("✅ Pushed commit to stack");
    info!("   Commit: {} ({})", &commit_hash[..8], commit_msg.split('\n').next().unwrap_or(""));
    info!("   Branch: {}", branch_name);
    info!("   Entry ID: {}", entry_id);

    Ok(())
}

async fn pop_from_stack(_keep_branch: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let mut manager = StackManager::new(&current_dir)?;
    
    let entry = manager.pop_from_stack()?;

    info!("✅ Popped commit from stack");
    info!("   Commit: {} ({})", entry.short_hash(), entry.short_message(50));
    info!("   Branch: {}", entry.branch);

    // TODO: Implement branch deletion if !keep_branch

    Ok(())
}

async fn submit_entry(_entry: Option<usize>, _title: Option<String>, _description: Option<String>) -> Result<()> {
    // TODO: Implement pull request creation in Phase 3
    warn!("Submit functionality will be implemented in Phase 3 (Bitbucket integration)");
    info!("For now, you can manually create pull requests for your stack entries");
    Ok(())
}

async fn sync_stack(_force: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let mut manager = StackManager::new(&current_dir)?;
    
    let active_stack = manager.get_active_stack()
        .ok_or_else(|| CascadeError::config("No active stack"))?;
    let stack_id = active_stack.id;

    manager.sync_stack(&stack_id)?;

    info!("✅ Stack synced successfully");

    Ok(())
}

async fn rebase_stack(_interactive: bool) -> Result<()> {
    // TODO: Implement rebase functionality in Phase 4
    warn!("Rebase functionality will be implemented in Phase 4 (Anti-Force-Push Strategy)");
    info!("For now, you can manually rebase your stack branches");
    Ok(())
}

async fn delete_stack(name: String, force: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let mut manager = StackManager::new(&current_dir)?;
    
    let stack = manager.get_stack_by_name(&name)
        .ok_or_else(|| CascadeError::config(format!("Stack '{}' not found", name)))?;
    let stack_id = stack.id;

    if !force && !stack.entries.is_empty() {
        return Err(CascadeError::config(
            format!("Stack '{}' has {} entries. Use --force to delete anyway", name, stack.entries.len())
        ));
    }

    let deleted = manager.delete_stack(&stack_id)?;

    info!("✅ Deleted stack '{}'", deleted.name);
    if !deleted.entries.is_empty() {
        warn!("   {} entries were removed", deleted.entries.len());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::process::Command;

    async fn create_test_repo() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repository
        Command::new("git")
            .args(&["init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
        Command::new("git")
            .args(&["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Initialize cascade
        crate::config::initialize_repo(&repo_path, Some("https://test.bitbucket.com".to_string())).unwrap();

        (temp_dir, repo_path)
    }

    #[tokio::test]
    async fn test_create_stack() {
        let (_temp_dir, repo_path) = create_test_repo().await;
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&repo_path).unwrap();

        let result = create_stack(
            "test-stack".to_string(),
            None, // Use default branch
            Some("Test description".to_string())
        ).await;

        let _ = env::set_current_dir(original_dir);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_empty_stacks() {
        let (_temp_dir, repo_path) = create_test_repo().await;
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&repo_path).unwrap();

        let result = list_stacks(false).await;

        let _ = env::set_current_dir(original_dir);
        assert!(result.is_ok());
    }
} 