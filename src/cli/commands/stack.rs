use crate::errors::{CascadeError, Result};
use crate::stack::{StackManager, StackStatus};
use crate::git::GitRepository;
use clap::{Subcommand, ValueEnum};
use std::env;
use tracing::{info, warn};
use indicatif::{ProgressBar, ProgressStyle, ProgressIterator};

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
        StackAction::Push { branch, message, commit } => {
            push_to_stack(branch, message, commit).await
        }
        StackAction::Pop { keep_branch } => {
            pop_from_stack(keep_branch).await
        }
        StackAction::Submit { entry, title, description } => {
            submit_entry(entry, title, description).await
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

async fn submit_entry(entry: Option<usize>, title: Option<String>, description: Option<String>) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    // Create progress bar for the submission process
    let pb = ProgressBar::new(5);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("📤 {msg} [{bar:40.cyan/blue}] {pos}/{len}")
            .map_err(|e| CascadeError::config(format!("Progress bar template error: {}", e)))?
    );

    pb.set_message("Loading configuration");
    let stack_manager = StackManager::new(&current_dir)?;
    pb.inc(1);
    
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

    pb.set_message("Finding stack entry");
    pb.inc(1);

    // Get the active stack
    let active_stack = stack_manager.get_active_stack()
        .ok_or_else(|| CascadeError::config("No active stack. Create a stack first with 'cc stack create'"))?;
    let stack_id = active_stack.id;

    // Determine which entry to submit
    let entry_to_submit = if let Some(entry_num) = entry {
        if entry_num == 0 || entry_num > active_stack.entries.len() {
            return Err(CascadeError::config(format!("Invalid entry number: {}. Stack has {} entries", entry_num, active_stack.entries.len())));
        }
        active_stack.entries[entry_num - 1].clone() // 1-based to 0-based indexing
    } else {
        // Default to the top entry (most recent)
        active_stack.entries.last()
            .ok_or_else(|| CascadeError::config("Stack is empty. Push some commits first with 'cc stack push'"))?
            .clone()
    };

    pb.set_message("Connecting to Bitbucket");
    pb.inc(1);
    
    // Create Bitbucket integration
    let mut integration = crate::bitbucket::BitbucketIntegration::new(stack_manager, cascade_config)?;
    
    pb.set_message("Creating pull request");
    pb.inc(1);
    
    // Submit the entry
    match integration.submit_entry(&stack_id, &entry_to_submit.id, title, description).await {
        Ok(pr) => {
            pb.set_message("Finalizing");
            pb.inc(1);
            pb.finish_with_message("✅ Pull request created successfully");

            println!("   PR #{}: {}", pr.id, pr.title);
            if let Some(url) = pr.web_url() {
                println!("   URL: {}", url);
            }
            println!("   From: {} -> {}", pr.from_ref.display_id, pr.to_ref.display_id);
        }
        Err(e) => {
            pb.abandon_with_message("❌ Failed to create pull request");
            return Err(CascadeError::config(format!("Failed to create pull request: {}", e)));
        }
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

    let stack_manager = StackManager::new(&current_dir)?;
    let git_repo = crate::git::GitRepository::open(&current_dir)?;
    
    // Get the active stack
    let active_stack = stack_manager.get_active_stack()
        .ok_or_else(|| CascadeError::config("No active stack"))?;
    let stack_id = active_stack.id;
    let stack_name = active_stack.name.clone();

    info!("🔄 Starting rebase for stack '{}'", stack_name);

    // Load configuration to determine rebase strategy
    let config_dir = crate::config::get_repo_config_dir(&current_dir)?;
    let config_path = config_dir.join("config.json");
    let settings = crate::config::Settings::load_from_file(&config_path)?;
    
    // Determine rebase strategy - CLI argument overrides config
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
            }
            
            println!("   ✅ {} commits successfully rebased", result.success_count());
            
            // Show next steps
            if matches!(rebase_strategy, crate::stack::RebaseStrategy::BranchVersioning) {
                println!("\n📝 Next steps:");
                println!("   1. Review the new branches created");
                println!("   2. Test your changes");
                println!("   3. Submit updated pull requests with 'cc stack submit'");
                println!("   4. Old branches are preserved for safety");
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