use crate::errors::{CascadeError, Result};
use crate::stack::{StackManager, StackStatus};
use crate::git::GitRepository;
use clap::{Subcommand, ValueEnum};
use std::env;
use tracing::{info, warn};
use indicatif::{ProgressBar, ProgressStyle, ProgressIterator};
use crate::bitbucket::BitbucketIntegration;

/// CLI argument version of RebaseStrategy
#[derive(ValueEnum, Clone, Debug)]
pub enum RebaseStrategyArg {
    /// Create new branches with version suffixes
    BranchVersioning,
    /// Use cherry-pick to apply commits
    CherryPick,
    /// Create merge commits
    ThreeWayMerge,
    /// Interactive rebase
    Interactive,
}

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
        /// Show only active stack
        #[arg(long)]
        active: bool,
        /// Output format (name, id, status)
        #[arg(long)]
        format: Option<String>,
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
        /// Push all unpushed commits since last stack push
        #[arg(long)]
        all: bool,
        /// Push commits since this reference (e.g., HEAD~3)
        #[arg(long)]
        since: Option<String>,
        /// Push multiple specific commits (comma-separated)
        #[arg(long)]
        commits: Option<String>,
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
        /// Submit all unsubmitted entries
        #[arg(long)]
        all: bool,
        /// Submit range of entries (e.g., "1-3" or "2,4,6")
        #[arg(long)]
        range: Option<String>,
    },
    
    /// Check status of all pull requests in a stack
    Status {
        /// Name of the stack (defaults to active stack)
        name: Option<String>,
    },
    
    /// List all pull requests for the repository
    Prs {
        /// Filter by state (open, merged, declined)
        #[arg(long)]
        state: Option<String>,
        /// Show detailed information
        #[arg(long, short)]
        verbose: bool,
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
        /// Target base branch (defaults to stack's base branch)
        #[arg(long)]
        onto: Option<String>,
        /// Rebase strategy to use
        #[arg(long, value_enum)]
        strategy: Option<RebaseStrategyArg>,
    },
    
    /// Continue an in-progress rebase after resolving conflicts
    ContinueRebase,
    
    /// Abort an in-progress rebase
    AbortRebase,
    
    /// Show rebase status and conflict resolution guidance
    RebaseStatus,
    
    /// Delete a stack
    Delete {
        /// Name of the stack to delete
        name: String,
        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Validate stack integrity
    Validate {
        /// Name of the stack (defaults to active stack)
        name: Option<String>,
    },
}

pub async fn run(action: StackAction) -> Result<()> {
    let _current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    match action {
        StackAction::Create { name, base, description } => {
            create_stack(name, base, description).await
        }
        StackAction::List { verbose, active, format } => {
            list_stacks(verbose, active, format).await
        }
        StackAction::Switch { name } => {
            switch_stack(name).await
        }
        StackAction::Show { name } => {
            show_stack(name).await
        }
        StackAction::Push { branch, message, commit, all, since, commits } => {
            push_to_stack(branch, message, commit, all, since, commits).await
        }
        StackAction::Pop { keep_branch } => {
            pop_from_stack(keep_branch).await
        }
        StackAction::Submit { entry, title, description, all, range } => {
            submit_entry(entry, title, description, all, range).await
        }
        StackAction::Status { name } => {
            check_stack_status(name).await
        }
        StackAction::Prs { state, verbose } => {
            list_pull_requests(state, verbose).await
        }
        StackAction::Sync { force } => {
            sync_stack(force).await
        }
        StackAction::Rebase { interactive, onto, strategy } => {
            rebase_stack(interactive, onto, strategy).await
        }
        StackAction::ContinueRebase => {
            continue_rebase().await
        }
        StackAction::AbortRebase => {
            abort_rebase().await
        }
        StackAction::RebaseStatus => {
            rebase_status().await
        }
        StackAction::Delete { name, force } => {
            delete_stack(name, force).await
        }
        StackAction::Validate { name } => {
            validate_stack(name).await
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

async fn list_stacks(verbose: bool, active: bool, format: Option<String>) -> Result<()> {
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
            StackStatus::NeedsSync => "🔄",
            StackStatus::Corrupted => "💥",
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
                println!("      PR: #{}", pr_id);
            }
        }
    } else {
        println!("\n📝 No entries yet. Use 'cc stack push' to add commits.");
    }

    Ok(())
}

async fn push_to_stack(branch: Option<String>, message: Option<String>, commit: Option<String>, all: bool, since: Option<String>, commits: Option<String>) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let mut manager = StackManager::new(&current_dir)?;
    let repo = GitRepository::open(&current_dir)?;

    // Determine which commits to push
    let commits_to_push = if let Some(commits_str) = commits {
        // Parse comma-separated commit hashes
        commits_str.split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<String>>()
    } else if let Some(since_ref) = since {
        // Get commits since the specified reference
        let since_commit = repo.resolve_reference(&since_ref)?;
        let head_commit = repo.get_head_commit()?;
        
        // Get commits between since_ref and HEAD
        let commits = repo.get_commits_between(&since_commit.id().to_string(), &head_commit.id().to_string())?;
        commits.into_iter()
            .map(|c| c.id().to_string())
            .collect()
    } else if all {
        // Get all unpushed commits (commits not in any stack entry)
        let active_stack = manager.get_active_stack()
            .ok_or_else(|| CascadeError::config("No active stack. Create a stack first with 'cc stack create'"))?;
        
        let mut unpushed = Vec::new();
        let head_commit = repo.get_head_commit()?;
        let mut current_commit = head_commit;
        
        // Walk back from HEAD until we find a commit that's already in the stack
        loop {
            let commit_hash = current_commit.id().to_string();
            let already_in_stack = active_stack.entries.iter()
                .any(|entry| entry.commit_hash == commit_hash);
            
            if already_in_stack {
                break;
            }
            
            unpushed.push(commit_hash);
            
            // Move to parent commit
            if let Some(parent) = current_commit.parents().next() {
                current_commit = parent;
            } else {
                break;
            }
        }
        
        unpushed.reverse(); // Reverse to get chronological order
        unpushed
    } else if let Some(hash) = commit {
        // Single specific commit
        vec![hash]
    } else {
        // Default: current HEAD
        vec![repo.get_head_commit()?.id().to_string()]
    };

    if commits_to_push.is_empty() {
        println!("ℹ️  No commits to push to stack");
        return Ok(());
    }

    // Push each commit to the stack
    let mut pushed_count = 0;
    for (i, commit_hash) in commits_to_push.iter().enumerate() {
        let commit_obj = repo.get_commit(commit_hash)?;
        let commit_msg = commit_obj.message().unwrap_or("").to_string();
        
        // Generate branch name (use provided branch for first commit, generate for others)
        let branch_name = if i == 0 && branch.is_some() {
            branch.clone().unwrap()
        } else {
            // Create a temporary GitRepository for branch name generation
            let temp_repo = GitRepository::open(&current_dir)?;
            let branch_mgr = crate::git::BranchManager::new(temp_repo);
            branch_mgr.generate_branch_name(&commit_msg)
        };

        // Use provided message for first commit, original message for others
        let final_message = if i == 0 && message.is_some() {
            message.clone().unwrap()
        } else {
            commit_msg.clone()
        };

        let entry_id = manager.push_to_stack(branch_name.clone(), commit_hash.clone(), final_message.clone())?;
        pushed_count += 1;

        println!("✅ Pushed commit {}/{} to stack", i + 1, commits_to_push.len());
        println!("   Commit: {} ({})", &commit_hash[..8], commit_msg.split('\n').next().unwrap_or(""));
        println!("   Branch: {}", branch_name);
        println!("   Entry ID: {}", entry_id);
        println!();
    }

    println!("🎉 Successfully pushed {} commit{} to stack", 
             pushed_count, 
             if pushed_count == 1 { "" } else { "s" });

    Ok(())
}

async fn pop_from_stack(keep_branch: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let mut manager = StackManager::new(&current_dir)?;
    let repo = GitRepository::open(&current_dir)?;
    
    let entry = manager.pop_from_stack()?;

    info!("✅ Popped commit from stack");
    info!("   Commit: {} ({})", entry.short_hash(), entry.short_message(50));
    info!("   Branch: {}", entry.branch);

    // Delete branch if requested and it's not the current branch
    if !keep_branch && entry.branch != repo.get_current_branch()? {
        match repo.delete_branch(&entry.branch) {
            Ok(_) => info!("   Deleted branch: {}", entry.branch),
            Err(e) => warn!("   Could not delete branch {}: {}", entry.branch, e),
        }
    }

    Ok(())
}

async fn submit_entry(entry: Option<usize>, title: Option<String>, description: Option<String>, all: bool, range: Option<String>) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    // Load configuration first
    let config_dir = crate::config::get_repo_config_dir(&current_dir)?;
    let config_path = config_dir.join("config.json");
    let settings = crate::config::Settings::load_from_file(&config_path)?;
    
    // Create the main config structure
    let cascade_config = crate::config::CascadeConfig {
        bitbucket: Some(settings.bitbucket.clone()),
        git: settings.git.clone(),
        auth: crate::config::AuthConfig::default(),
    };

    let stack_manager = StackManager::new(&current_dir)?;
    
    // Get the active stack
    let active_stack = stack_manager.get_active_stack()
        .ok_or_else(|| CascadeError::config("No active stack. Create a stack first with 'cc stack create'"))?;
    let stack_id = active_stack.id;

    // Determine which entries to submit
    let entries_to_submit = if all {
        // Submit all unsubmitted entries
        active_stack.entries.iter()
            .enumerate()
            .filter(|(_, entry)| !entry.is_submitted)
            .map(|(i, entry)| (i + 1, entry.clone())) // Convert to 1-based indexing
            .collect::<Vec<(usize, _)>>()
    } else if let Some(range_str) = range {
        // Parse range (e.g., "1-3" or "2,4,6")
        let mut entries = Vec::new();
        
        if range_str.contains('-') {
            // Handle range like "1-3"
            let parts: Vec<&str> = range_str.split('-').collect();
            if parts.len() != 2 {
                return Err(CascadeError::config("Invalid range format. Use 'start-end' (e.g., '1-3')"));
            }
            
            let start: usize = parts[0].parse()
                .map_err(|_| CascadeError::config("Invalid start number in range"))?;
            let end: usize = parts[1].parse()
                .map_err(|_| CascadeError::config("Invalid end number in range"))?;
            
            if start == 0 || end == 0 || start > active_stack.entries.len() || end > active_stack.entries.len() {
                return Err(CascadeError::config(format!("Range out of bounds. Stack has {} entries", active_stack.entries.len())));
            }
            
            for i in start..=end {
                entries.push((i, active_stack.entries[i - 1].clone()));
            }
        } else {
            // Handle comma-separated list like "2,4,6"
            for entry_str in range_str.split(',') {
                let entry_num: usize = entry_str.trim().parse()
                    .map_err(|_| CascadeError::config(format!("Invalid entry number: {}", entry_str)))?;
                
                if entry_num == 0 || entry_num > active_stack.entries.len() {
                    return Err(CascadeError::config(format!("Entry {} out of bounds. Stack has {} entries", entry_num, active_stack.entries.len())));
                }
                
                entries.push((entry_num, active_stack.entries[entry_num - 1].clone()));
            }
        }
        
        entries
    } else if let Some(entry_num) = entry {
        // Single entry specified
        if entry_num == 0 || entry_num > active_stack.entries.len() {
            return Err(CascadeError::config(format!("Invalid entry number: {}. Stack has {} entries", entry_num, active_stack.entries.len())));
        }
        vec![(entry_num, active_stack.entries[entry_num - 1].clone())]
    } else {
        // Default to the top entry (most recent)
        let top_entry = active_stack.entries.last()
            .ok_or_else(|| CascadeError::config("Stack is empty. Push some commits first with 'cc stack push'"))?;
        vec![(active_stack.entries.len(), top_entry.clone())]
    };

    if entries_to_submit.is_empty() {
        println!("ℹ️  No entries to submit");
        return Ok(());
    }

    // Create progress bar for the submission process
    let total_operations = entries_to_submit.len() + 2; // +2 for setup steps
    let pb = ProgressBar::new(total_operations as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("📤 {msg} [{bar:40.cyan/blue}] {pos}/{len}")
            .map_err(|e| CascadeError::config(format!("Progress bar template error: {}", e)))?
    );

    pb.set_message("Connecting to Bitbucket");
    pb.inc(1);
    
    // Create a new StackManager for the integration (since the original was moved)
    let integration_stack_manager = StackManager::new(&current_dir)?;
    let mut integration = BitbucketIntegration::new(integration_stack_manager, cascade_config.clone())?;
    
    pb.set_message("Starting batch submission");
    pb.inc(1);
    
    // Submit each entry
    let mut submitted_count = 0;
    let mut failed_entries = Vec::new();
    let total_entries = entries_to_submit.len();
    
    for (entry_num, entry_to_submit) in &entries_to_submit {
        pb.set_message("Submitting entries...");
        
        // Use provided title/description only for first entry or single entry submissions
        let entry_title = if total_entries == 1 { title.clone() } else { None };
        let entry_description = if total_entries == 1 { description.clone() } else { None };
        
        match integration.submit_entry(&stack_id, &entry_to_submit.id, entry_title, entry_description).await {
            Ok(pr) => {
                submitted_count += 1;
                println!("✅ Entry {} - PR #{}: {}", entry_num, pr.id, pr.title);
                if let Some(url) = pr.web_url() {
                    println!("   URL: {}", url);
                }
                println!("   From: {} -> {}", pr.from_ref.display_id, pr.to_ref.display_id);
                println!();
            }
            Err(e) => {
                failed_entries.push((*entry_num, e.to_string()));
                println!("❌ Entry {} failed: {}", entry_num, e);
            }
        }
        
        pb.inc(1);
    }

    if failed_entries.is_empty() {
        pb.finish_with_message("✅ All pull requests created successfully");
        println!("🎉 Successfully submitted {} entr{}", 
                 submitted_count, 
                 if submitted_count == 1 { "y" } else { "ies" });
    } else {
        pb.abandon_with_message("⚠️  Some submissions failed");
        println!("📊 Submission Summary:");
        println!("   ✅ Successful: {}", submitted_count);
        println!("   ❌ Failed: {}", failed_entries.len());
        println!();
        println!("💡 Failed entries:");
        for (entry_num, error) in failed_entries {
            println!("   - Entry {}: {}", entry_num, error);
        }
        println!();
        println!("💡 You can retry failed entries individually:");
        println!("   cc stack submit <ENTRY_NUMBER>");
    }

    Ok(())
}

async fn check_stack_status(name: Option<String>) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let stack_manager = StackManager::new(&current_dir)?;
    
    // Load configuration
    let config_dir = crate::config::get_repo_config_dir(&current_dir)?;
    let config_path = config_dir.join("config.json");
    let settings = crate::config::Settings::load_from_file(&config_path)?;
    
    // Create the main config structure
    let cascade_config = crate::config::CascadeConfig {
        bitbucket: Some(settings.bitbucket.clone()),
        git: settings.git.clone(),
        auth: crate::config::AuthConfig::default(),
    };
    
    // Get stack information BEFORE moving stack_manager
    let stack = if let Some(name) = name {
        stack_manager.get_stack_by_name(&name)
            .ok_or_else(|| CascadeError::config(format!("Stack '{}' not found", name)))?
    } else {
        stack_manager.get_active_stack()
            .ok_or_else(|| CascadeError::config("No active stack. Use 'cc stack list' to see available stacks"))?
    };
    let stack_id = stack.id;

    println!("📋 Stack: {}", stack.name);
    println!("   ID: {}", stack.id);
    println!("   Base: {}", stack.base_branch);
    
    if let Some(description) = &stack.description {
        println!("   Description: {}", description);
    }

    // Create Bitbucket integration (this takes ownership of stack_manager)
    let integration = crate::bitbucket::BitbucketIntegration::new(stack_manager, cascade_config)?;
    
    // Check stack status
    match integration.check_stack_status(&stack_id).await {
        Ok(status) => {
            println!("\n📊 Pull Request Status:");
            println!("   Total entries: {}", status.total_entries);
            println!("   Submitted: {}", status.submitted_entries);
            println!("   Open PRs: {}", status.open_prs);
            println!("   Merged PRs: {}", status.merged_prs);
            println!("   Declined PRs: {}", status.declined_prs);
            println!("   Completion: {:.1}%", status.completion_percentage());

            if !status.pull_requests.is_empty() {
                println!("\n📋 Pull Requests:");
                for pr in &status.pull_requests {
                    let state_icon = match pr.state {
                        crate::bitbucket::PullRequestState::Open => "🔄",
                        crate::bitbucket::PullRequestState::Merged => "✅",
                        crate::bitbucket::PullRequestState::Declined => "❌",
                    };
                    println!("   {} PR #{}: {} ({} -> {})", 
                        state_icon, pr.id, pr.title, pr.from_ref.display_id, pr.to_ref.display_id);
                    if let Some(url) = pr.web_url() {
                        println!("      URL: {}", url);
                    }
                }
            }
        }
        Err(e) => {
            warn!("Failed to check stack status: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

async fn list_pull_requests(state: Option<String>, verbose: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let stack_manager = StackManager::new(&current_dir)?;
    
    // Load configuration
    let config_dir = crate::config::get_repo_config_dir(&current_dir)?;
    let config_path = config_dir.join("config.json");
    let settings = crate::config::Settings::load_from_file(&config_path)?;
    
    // Create the main config structure
    let cascade_config = crate::config::CascadeConfig {
        bitbucket: Some(settings.bitbucket.clone()),
        git: settings.git.clone(),
        auth: crate::config::AuthConfig::default(),
    };

    // Create Bitbucket integration
    let integration = crate::bitbucket::BitbucketIntegration::new(stack_manager, cascade_config)?;
    
    // Parse state filter
    let pr_state = if let Some(state_str) = state {
        match state_str.to_lowercase().as_str() {
            "open" => Some(crate::bitbucket::PullRequestState::Open),
            "merged" => Some(crate::bitbucket::PullRequestState::Merged),
            "declined" => Some(crate::bitbucket::PullRequestState::Declined),
            _ => return Err(CascadeError::config(format!("Invalid state '{}'. Use: open, merged, declined", state_str))),
        }
    } else {
        None
    };

    // Get pull requests
    match integration.list_pull_requests(pr_state).await {
        Ok(pr_page) => {
            if pr_page.values.is_empty() {
                info!("No pull requests found.");
                return Ok(());
            }

            println!("📋 Pull Requests ({} total):", pr_page.values.len());
            for pr in &pr_page.values {
                let state_icon = match pr.state {
                    crate::bitbucket::PullRequestState::Open => "🔄",
                    crate::bitbucket::PullRequestState::Merged => "✅",
                    crate::bitbucket::PullRequestState::Declined => "❌",
                };
                println!("   {} PR #{}: {}", state_icon, pr.id, pr.title);
                if verbose {
                    println!("      From: {} -> {}", pr.from_ref.display_id, pr.to_ref.display_id);
                    println!("      Author: {}", pr.author.user.display_name);
                    if let Some(url) = pr.web_url() {
                        println!("      URL: {}", url);
                    }
                    if let Some(desc) = &pr.description {
                        if !desc.is_empty() {
                            println!("      Description: {}", desc);
                        }
                    }
                    println!();
                }
            }

            if !verbose {
                println!("\nUse --verbose for more details");
            }
        }
        Err(e) => {
            warn!("Failed to list pull requests: {}", e);
            return Err(e);
        }
    }

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

async fn rebase_stack(interactive: bool, onto: Option<String>, strategy: Option<RebaseStrategyArg>) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let mut stack_manager = StackManager::new(&current_dir)?;
    let git_repo = GitRepository::open(&current_dir)?;
    
    // Load configuration for potential Bitbucket integration
    let config_dir = crate::config::get_repo_config_dir(&current_dir)?;
    let config_path = config_dir.join("config.json");
    let settings = crate::config::Settings::load_from_file(&config_path)?;
    
    // Create the main config structure
    let cascade_config = crate::config::CascadeConfig {
        bitbucket: Some(settings.bitbucket.clone()),
        git: settings.git.clone(),
        auth: crate::config::AuthConfig::default(),
    };

    // Get active stack
    let active_stack = stack_manager.get_active_stack()
        .ok_or_else(|| CascadeError::config("No active stack. Create a stack first with 'cc stack create'"))?;
    let stack_id = active_stack.id;

    let active_stack = stack_manager.get_stack(&stack_id)
        .ok_or_else(|| CascadeError::config("Active stack not found"))?
        .clone();

    if active_stack.entries.is_empty() {
        println!("ℹ️  Stack is empty. Nothing to rebase.");
        return Ok(());
    }

    println!("🔄 Rebasing stack: {}", active_stack.name);
    println!("   Base: {}", active_stack.base_branch);
    
    // Determine rebase strategy
    let rebase_strategy = if let Some(cli_strategy) = strategy {
        match cli_strategy {
            RebaseStrategyArg::BranchVersioning => crate::stack::RebaseStrategy::BranchVersioning,
            RebaseStrategyArg::CherryPick => crate::stack::RebaseStrategy::CherryPick,
            RebaseStrategyArg::ThreeWayMerge => crate::stack::RebaseStrategy::ThreeWayMerge,
            RebaseStrategyArg::Interactive => crate::stack::RebaseStrategy::Interactive,
        }
    } else {
        // Use strategy from config
        match settings.cascade.default_sync_strategy.as_str() {
            "branch-versioning" => crate::stack::RebaseStrategy::BranchVersioning,
            "cherry-pick" => crate::stack::RebaseStrategy::CherryPick,
            "three-way-merge" => crate::stack::RebaseStrategy::ThreeWayMerge,
            "rebase" => crate::stack::RebaseStrategy::Interactive,
            _ => crate::stack::RebaseStrategy::BranchVersioning, // default fallback
        }
    };

    // Create rebase options
    let options = crate::stack::RebaseOptions {
        strategy: rebase_strategy.clone(),
        interactive,
        target_base: onto,
        preserve_merges: true,
        auto_resolve: !interactive, // Auto-resolve unless interactive
        max_retries: 3,
    };

    info!("   Strategy: {:?}", rebase_strategy);
    info!("   Interactive: {}", interactive);
    info!("   Target base: {:?}", options.target_base);
    info!("   Entries: {}", active_stack.entries.len());

    // Check if there's already a rebase in progress
    let mut rebase_manager = crate::stack::RebaseManager::new(stack_manager, git_repo, options);
    
    if rebase_manager.is_rebase_in_progress() {
        println!("⚠️  Rebase already in progress!");
        println!("   Use 'git status' to check the current state");
        println!("   Use 'cc stack continue-rebase' to continue");
        println!("   Use 'cc stack abort-rebase' to abort");
        return Ok(());
    }

    // Perform the rebase
    match rebase_manager.rebase_stack(&stack_id) {
        Ok(result) => {
            println!("🎉 Rebase completed!");
            println!("   {}", result.get_summary());
            
            if result.has_conflicts() {
                println!("   ⚠️  {} conflicts were resolved", result.conflicts.len());
                for conflict in &result.conflicts {
                    println!("      - {}", &conflict[..8.min(conflict.len())]);
                }
            }
            
            if !result.branch_mapping.is_empty() {
                println!("   📋 Branch mapping:");
                for (old, new) in &result.branch_mapping {
                    println!("      {} -> {}", old, new);
                }
                
                // Handle PR updates if enabled
                if let Some(ref bitbucket_config) = cascade_config.bitbucket {
                    // Create a new StackManager for the integration (since the original was moved)
                    let integration_stack_manager = StackManager::new(&current_dir)?;
                    let mut integration = BitbucketIntegration::new(integration_stack_manager, cascade_config.clone())?;
                    
                    match integration.update_prs_after_rebase(&stack_id, &result.branch_mapping).await {
                        Ok(updated_prs) => {
                            if !updated_prs.is_empty() {
                                println!("   🔄 Preserved pull request history:");
                                for pr_update in updated_prs {
                                    println!("      ✅ {}", pr_update);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("   ⚠️  Failed to update pull requests: {}", e);
                            eprintln!("      You may need to manually update PRs in Bitbucket");
                        }
                    }
                }
            }
            
            println!("   ✅ {} commits successfully rebased", result.success_count());
            
            // Show next steps
            if matches!(rebase_strategy, crate::stack::RebaseStrategy::BranchVersioning) {
                println!("\n📝 Next steps:");
                if !result.branch_mapping.is_empty() {
                    println!("   1. ✅ New versioned branches have been created");
                    println!("   2. ✅ Pull requests have been updated automatically");
                    println!("   3. 🔍 Review the updated PRs in Bitbucket");
                    println!("   4. 🧪 Test your changes on the new branches");
                    println!("   5. 🗑️  Old branches are preserved for safety (can be deleted later)");
                } else {
                    println!("   1. Review the rebased stack");
                    println!("   2. Test your changes");
                    println!("   3. Submit new pull requests with 'cc stack submit'");
                }
            }
        }
        Err(e) => {
            warn!("❌ Rebase failed: {}", e);
            println!("💡 Tips for resolving rebase issues:");
            println!("   - Check for uncommitted changes with 'git status'");
            println!("   - Ensure base branch is up to date");
            println!("   - Try interactive mode: 'cc stack rebase --interactive'");
            return Err(e);
        }
    }

    Ok(())
}

async fn continue_rebase() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let stack_manager = StackManager::new(&current_dir)?;
    let git_repo = crate::git::GitRepository::open(&current_dir)?;
    let options = crate::stack::RebaseOptions::default();
    let rebase_manager = crate::stack::RebaseManager::new(stack_manager, git_repo, options);
    
    if !rebase_manager.is_rebase_in_progress() {
        println!("ℹ️  No rebase in progress");
        return Ok(());
    }

    println!("🔄 Continuing rebase...");
    match rebase_manager.continue_rebase() {
        Ok(_) => {
            println!("✅ Rebase continued successfully");
            println!("   Check 'cc stack rebase-status' for current state");
        }
        Err(e) => {
            warn!("❌ Failed to continue rebase: {}", e);
            println!("💡 You may need to resolve conflicts first:");
            println!("   1. Edit conflicted files");
            println!("   2. Stage resolved files with 'git add'");
            println!("   3. Run 'cc stack continue-rebase' again");
        }
    }

    Ok(())
}

async fn abort_rebase() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let stack_manager = StackManager::new(&current_dir)?;
    let git_repo = crate::git::GitRepository::open(&current_dir)?;
    let options = crate::stack::RebaseOptions::default();
    let rebase_manager = crate::stack::RebaseManager::new(stack_manager, git_repo, options);
    
    if !rebase_manager.is_rebase_in_progress() {
        println!("ℹ️  No rebase in progress");
        return Ok(());
    }

    println!("⚠️  Aborting rebase...");
    match rebase_manager.abort_rebase() {
        Ok(_) => {
            println!("✅ Rebase aborted successfully");
            println!("   Repository restored to pre-rebase state");
        }
        Err(e) => {
            warn!("❌ Failed to abort rebase: {}", e);
            println!("⚠️  You may need to manually clean up the repository state");
        }
    }

    Ok(())
}

async fn rebase_status() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let stack_manager = StackManager::new(&current_dir)?;
    let git_repo = crate::git::GitRepository::open(&current_dir)?;
    
    println!("📊 Rebase Status");
    
    // Check if rebase is in progress by checking git state directly
    let git_dir = current_dir.join(".git");
    let rebase_in_progress = git_dir.join("REBASE_HEAD").exists() || 
        git_dir.join("rebase-merge").exists() ||
        git_dir.join("rebase-apply").exists();
    
    if rebase_in_progress {
        println!("   Status: 🔄 Rebase in progress");
        println!("   
📝 Actions available:");
        println!("     - 'cc stack continue-rebase' to continue");
        println!("     - 'cc stack abort-rebase' to abort");
        println!("     - 'git status' to see conflicted files");
        
        // Check for conflicts
        match git_repo.get_status() {
            Ok(statuses) => {
                let mut conflicts = Vec::new();
                for status in statuses.iter() {
                    if status.status().contains(git2::Status::CONFLICTED) {
                        if let Some(path) = status.path() {
                            conflicts.push(path.to_string());
                        }
                    }
                }
                
                if !conflicts.is_empty() {
                    println!("   ⚠️  Conflicts in {} files:", conflicts.len());
                    for conflict in conflicts {
                        println!("      - {}", conflict);
                    }
                    println!("   
💡 To resolve conflicts:");
                    println!("     1. Edit the conflicted files");
                    println!("     2. Stage resolved files: git add <file>");
                    println!("     3. Continue: cc stack continue-rebase");
                }
            }
            Err(e) => {
                warn!("Failed to get git status: {}", e);
            }
        }
    } else {
        println!("   Status: ✅ No rebase in progress");
        
        // Show stack status instead
        if let Some(active_stack) = stack_manager.get_active_stack() {
            println!("   Active stack: {}", active_stack.name);
            println!("   Entries: {}", active_stack.entries.len());
            println!("   Base branch: {}", active_stack.base_branch);
        }
    }

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

async fn validate_stack(name: Option<String>) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    let manager = StackManager::new(&current_dir)?;
    
    if let Some(name) = name {
        // Validate specific stack
        let stack = manager.get_stack_by_name(&name)
            .ok_or_else(|| CascadeError::config(format!("Stack '{}' not found", name)))?;
        
        match stack.validate() {
            Ok(_) => {
                println!("✅ Stack '{}' validation passed", name);
                Ok(())
            }
            Err(e) => {
                println!("❌ Stack '{}' validation failed: {}", name, e);
                Err(CascadeError::config(e))
            }
        }
    } else {
        // Validate all stacks
        match manager.validate_all() {
            Ok(_) => {
                println!("✅ All stacks validation passed");
                Ok(())
            }
            Err(e) => {
                println!("❌ Stack validation failed: {}", e);
                Err(e)
            }
        }
    }
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

        let result = list_stacks(false, false, None).await;

        let _ = env::set_current_dir(original_dir);
        assert!(result.is_ok());
    }
} 