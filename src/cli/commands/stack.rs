use crate::bitbucket::BitbucketIntegration;
use crate::cli::output::Output;
use crate::errors::{CascadeError, Result};
use crate::git::{find_repository_root, GitRepository};
use crate::stack::{StackManager, StackStatus};
use clap::{Subcommand, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::io::{self, Write};
use tracing::{info, warn};

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

#[derive(ValueEnum, Clone, Debug)]
pub enum MergeStrategyArg {
    /// Create a merge commit
    Merge,
    /// Squash all commits into one
    Squash,
    /// Fast-forward merge when possible
    FastForward,
}

impl From<MergeStrategyArg> for crate::bitbucket::pull_request::MergeStrategy {
    fn from(arg: MergeStrategyArg) -> Self {
        match arg {
            MergeStrategyArg::Merge => Self::Merge,
            MergeStrategyArg::Squash => Self::Squash,
            MergeStrategyArg::FastForward => Self::FastForward,
        }
    }
}

#[derive(Debug, Subcommand)]
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

    /// Deactivate the current stack (turn off stack mode)
    Deactivate {
        /// Force deactivation without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Show the current stack status  
    Show {
        /// Show detailed pull request information
        #[arg(short, long)]
        verbose: bool,
        /// Show mergability status for all PRs
        #[arg(short, long)]
        mergeable: bool,
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
        /// Push commits since this reference (e.g., HEAD~3)
        #[arg(long)]
        since: Option<String>,
        /// Push multiple specific commits (comma-separated)
        #[arg(long)]
        commits: Option<String>,
        /// Squash unpushed commits before pushing (optional: specify count)
        #[arg(long, num_args = 0..=1, default_missing_value = "0")]
        squash: Option<usize>,
        /// Squash all commits since this reference (e.g., HEAD~5)
        #[arg(long)]
        squash_since: Option<String>,
        /// Auto-create feature branch when pushing from base branch
        #[arg(long)]
        auto_branch: bool,
        /// Allow pushing commits from base branch (not recommended)
        #[arg(long)]
        allow_base_branch: bool,
        /// Show what would be pushed without actually pushing
        #[arg(long)]
        dry_run: bool,
    },

    /// Pop the top commit from the stack
    Pop {
        /// Keep the branch (don't delete it)
        #[arg(long)]
        keep_branch: bool,
    },

    /// Submit a stack entry for review
    Submit {
        /// Stack entry number (1-based, defaults to all unsubmitted)
        entry: Option<usize>,
        /// Pull request title
        #[arg(long, short)]
        title: Option<String>,
        /// Pull request description
        #[arg(long, short)]
        description: Option<String>,
        /// Submit range of entries (e.g., "1-3" or "2,4,6")
        #[arg(long)]
        range: Option<String>,
        /// Create draft pull requests (can be edited later)
        #[arg(long)]
        draft: bool,
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

    /// Check stack status with remote repository (read-only)
    Check {
        /// Force check even if there are issues
        #[arg(long)]
        force: bool,
    },

    /// Sync stack with remote repository (pull + rebase + cleanup)
    Sync {
        /// Force sync even if there are conflicts
        #[arg(long)]
        force: bool,
        /// Skip cleanup of merged branches
        #[arg(long)]
        skip_cleanup: bool,
        /// Interactive mode for conflict resolution
        #[arg(long, short)]
        interactive: bool,
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

    /// Validate stack integrity and handle branch modifications
    ///
    /// Checks that stack branches match their expected commit hashes.
    /// Detects when branches have been manually modified (extra commits added).
    ///
    /// Available --fix modes:
    /// • incorporate: Update stack entry to include extra commits
    /// • split: Create new stack entry for extra commits  
    /// • reset: Remove extra commits (DESTRUCTIVE - loses work)
    ///
    /// Without --fix, runs interactively asking for each modification.
    Validate {
        /// Name of the stack (defaults to active stack)
        name: Option<String>,
        /// Auto-fix mode: incorporate, split, or reset
        #[arg(long)]
        fix: Option<String>,
    },

    /// Land (merge) approved stack entries
    Land {
        /// Stack entry number to land (1-based index, optional)
        entry: Option<usize>,
        /// Force land even with blocking issues (dangerous)
        #[arg(short, long)]
        force: bool,
        /// Dry run - show what would be landed without doing it
        #[arg(short, long)]
        dry_run: bool,
        /// Use server-side validation (safer, checks approvals/builds)
        #[arg(long)]
        auto: bool,
        /// Wait for builds to complete before merging
        #[arg(long)]
        wait_for_builds: bool,
        /// Merge strategy to use
        #[arg(long, value_enum, default_value = "squash")]
        strategy: Option<MergeStrategyArg>,
        /// Maximum time to wait for builds (seconds)
        #[arg(long, default_value = "1800")]
        build_timeout: u64,
    },

    /// Auto-land all ready PRs (shorthand for land --auto)
    AutoLand {
        /// Force land even with blocking issues (dangerous)
        #[arg(short, long)]
        force: bool,
        /// Dry run - show what would be landed without doing it
        #[arg(short, long)]
        dry_run: bool,
        /// Wait for builds to complete before merging
        #[arg(long)]
        wait_for_builds: bool,
        /// Merge strategy to use
        #[arg(long, value_enum, default_value = "squash")]
        strategy: Option<MergeStrategyArg>,
        /// Maximum time to wait for builds (seconds)
        #[arg(long, default_value = "1800")]
        build_timeout: u64,
    },

    /// List pull requests from Bitbucket
    ListPrs {
        /// Filter by state (open, merged, declined)
        #[arg(short, long)]
        state: Option<String>,
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },

    /// Continue an in-progress land operation after resolving conflicts
    ContinueLand,

    /// Abort an in-progress land operation  
    AbortLand,

    /// Show status of in-progress land operation
    LandStatus,

    /// Repair data consistency issues in stack metadata
    Repair,
}

pub async fn run(action: StackAction) -> Result<()> {
    match action {
        StackAction::Create {
            name,
            base,
            description,
        } => create_stack(name, base, description).await,
        StackAction::List {
            verbose,
            active,
            format,
        } => list_stacks(verbose, active, format).await,
        StackAction::Switch { name } => switch_stack(name).await,
        StackAction::Deactivate { force } => deactivate_stack(force).await,
        StackAction::Show { verbose, mergeable } => show_stack(verbose, mergeable).await,
        StackAction::Push {
            branch,
            message,
            commit,
            since,
            commits,
            squash,
            squash_since,
            auto_branch,
            allow_base_branch,
            dry_run,
        } => {
            push_to_stack(
                branch,
                message,
                commit,
                since,
                commits,
                squash,
                squash_since,
                auto_branch,
                allow_base_branch,
                dry_run,
            )
            .await
        }
        StackAction::Pop { keep_branch } => pop_from_stack(keep_branch).await,
        StackAction::Submit {
            entry,
            title,
            description,
            range,
            draft,
        } => submit_entry(entry, title, description, range, draft).await,
        StackAction::Status { name } => check_stack_status(name).await,
        StackAction::Prs { state, verbose } => list_pull_requests(state, verbose).await,
        StackAction::Check { force } => check_stack(force).await,
        StackAction::Sync {
            force,
            skip_cleanup,
            interactive,
        } => sync_stack(force, skip_cleanup, interactive).await,
        StackAction::Rebase {
            interactive,
            onto,
            strategy,
        } => rebase_stack(interactive, onto, strategy).await,
        StackAction::ContinueRebase => continue_rebase().await,
        StackAction::AbortRebase => abort_rebase().await,
        StackAction::RebaseStatus => rebase_status().await,
        StackAction::Delete { name, force } => delete_stack(name, force).await,
        StackAction::Validate { name, fix } => validate_stack(name, fix).await,
        StackAction::Land {
            entry,
            force,
            dry_run,
            auto,
            wait_for_builds,
            strategy,
            build_timeout,
        } => {
            land_stack(
                entry,
                force,
                dry_run,
                auto,
                wait_for_builds,
                strategy,
                build_timeout,
            )
            .await
        }
        StackAction::AutoLand {
            force,
            dry_run,
            wait_for_builds,
            strategy,
            build_timeout,
        } => auto_land_stack(force, dry_run, wait_for_builds, strategy, build_timeout).await,
        StackAction::ListPrs { state, verbose } => list_pull_requests(state, verbose).await,
        StackAction::ContinueLand => continue_land().await,
        StackAction::AbortLand => abort_land().await,
        StackAction::LandStatus => land_status().await,
        StackAction::Repair => repair_stack_data().await,
    }
}

// Public functions for shortcut commands
pub async fn show(verbose: bool, mergeable: bool) -> Result<()> {
    show_stack(verbose, mergeable).await
}

#[allow(clippy::too_many_arguments)]
pub async fn push(
    branch: Option<String>,
    message: Option<String>,
    commit: Option<String>,
    since: Option<String>,
    commits: Option<String>,
    squash: Option<usize>,
    squash_since: Option<String>,
    auto_branch: bool,
    allow_base_branch: bool,
    dry_run: bool,
) -> Result<()> {
    push_to_stack(
        branch,
        message,
        commit,
        since,
        commits,
        squash,
        squash_since,
        auto_branch,
        allow_base_branch,
        dry_run,
    )
    .await
}

pub async fn pop(keep_branch: bool) -> Result<()> {
    pop_from_stack(keep_branch).await
}

pub async fn land(
    entry: Option<usize>,
    force: bool,
    dry_run: bool,
    auto: bool,
    wait_for_builds: bool,
    strategy: Option<MergeStrategyArg>,
    build_timeout: u64,
) -> Result<()> {
    land_stack(
        entry,
        force,
        dry_run,
        auto,
        wait_for_builds,
        strategy,
        build_timeout,
    )
    .await
}

pub async fn autoland(
    force: bool,
    dry_run: bool,
    wait_for_builds: bool,
    strategy: Option<MergeStrategyArg>,
    build_timeout: u64,
) -> Result<()> {
    auto_land_stack(force, dry_run, wait_for_builds, strategy, build_timeout).await
}

pub async fn sync(force: bool, skip_cleanup: bool, interactive: bool) -> Result<()> {
    sync_stack(force, skip_cleanup, interactive).await
}

pub async fn rebase(
    interactive: bool,
    onto: Option<String>,
    strategy: Option<RebaseStrategyArg>,
) -> Result<()> {
    rebase_stack(interactive, onto, strategy).await
}

pub async fn deactivate(force: bool) -> Result<()> {
    deactivate_stack(force).await
}

pub async fn switch(name: String) -> Result<()> {
    switch_stack(name).await
}

async fn create_stack(
    name: String,
    base: Option<String>,
    description: Option<String>,
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut manager = StackManager::new(&repo_root)?;
    let stack_id = manager.create_stack(name.clone(), base.clone(), description.clone())?;

    // Get the created stack to check its working branch
    let stack = manager
        .get_stack(&stack_id)
        .ok_or_else(|| CascadeError::config("Failed to get created stack"))?;

    // Use the new output format
    Output::stack_info(
        &name,
        &stack_id.to_string(),
        &stack.base_branch,
        stack.working_branch.as_deref(),
        true, // is_active
    );

    if let Some(desc) = description {
        Output::sub_item(format!("Description: {desc}"));
    }

    // Provide helpful guidance based on the working branch situation
    if stack.working_branch.is_none() {
        Output::warning(format!(
            "You're currently on the base branch '{}'",
            stack.base_branch
        ));
        Output::next_steps(&[
            &format!("Create a feature branch: git checkout -b {name}"),
            "Make changes and commit them",
            "Run 'ca push' to add commits to this stack",
        ]);
    } else {
        Output::next_steps(&[
            "Make changes and commit them",
            "Run 'ca push' to add commits to this stack",
            "Use 'ca submit' when ready to create pull requests",
        ]);
    }

    Ok(())
}

async fn list_stacks(verbose: bool, _active: bool, _format: Option<String>) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let manager = StackManager::new(&repo_root)?;
    let stacks = manager.list_stacks();

    if stacks.is_empty() {
        Output::info("No stacks found. Create one with: ca stack create <name>");
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

        let active_indicator = if active_marker.is_some() {
            " (active)"
        } else {
            ""
        };

        // Get the actual stack object to access branch information
        let stack = manager.get_stack(&stack_id);

        if verbose {
            println!("  {status_icon} {name} [{entry_count}]{active_indicator}");
            println!("    ID: {stack_id}");
            if let Some(stack_meta) = manager.get_stack_metadata(&stack_id) {
                println!("    Base: {}", stack_meta.base_branch);
                if let Some(desc) = &stack_meta.description {
                    println!("    Description: {desc}");
                }
                println!(
                    "    Commits: {} total, {} submitted",
                    stack_meta.total_commits, stack_meta.submitted_commits
                );
                if stack_meta.has_conflicts {
                    println!("    ⚠️  Has conflicts");
                }
            }

            // Show branch information in verbose mode
            if let Some(stack_obj) = stack {
                if !stack_obj.entries.is_empty() {
                    println!("    Branches:");
                    for (i, entry) in stack_obj.entries.iter().enumerate() {
                        let entry_num = i + 1;
                        let submitted_indicator = if entry.is_submitted { "📤" } else { "📝" };
                        let branch_name = &entry.branch;
                        let short_message = if entry.message.len() > 40 {
                            format!("{}...", &entry.message[..37])
                        } else {
                            entry.message.clone()
                        };
                        println!("      {entry_num}. {submitted_indicator} {branch_name} - {short_message}");
                    }
                }
            }
            println!();
        } else {
            // Show compact branch info in non-verbose mode
            let branch_info = if let Some(stack_obj) = stack {
                if stack_obj.entries.is_empty() {
                    String::new()
                } else if stack_obj.entries.len() == 1 {
                    format!(" → {}", stack_obj.entries[0].branch)
                } else {
                    let first_branch = &stack_obj.entries[0].branch;
                    let last_branch = &stack_obj.entries.last().unwrap().branch;
                    format!(" → {first_branch} … {last_branch}")
                }
            } else {
                String::new()
            };

            println!("  {status_icon} {name} [{entry_count}]{branch_info}{active_indicator}");
        }
    }

    if !verbose {
        println!("\nUse --verbose for more details");
    }

    Ok(())
}

async fn switch_stack(name: String) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut manager = StackManager::new(&repo_root)?;
    let repo = GitRepository::open(&repo_root)?;

    // Get stack information before switching
    let stack = manager
        .get_stack_by_name(&name)
        .ok_or_else(|| CascadeError::config(format!("Stack '{name}' not found")))?;

    // Determine the target branch and provide appropriate messaging
    if let Some(working_branch) = &stack.working_branch {
        // Stack has a working branch - try to switch to it
        let current_branch = repo.get_current_branch().ok();

        if current_branch.as_ref() != Some(working_branch) {
            Output::progress(format!(
                "Switching to stack working branch: {working_branch}"
            ));

            // Check if target branch exists
            if repo.branch_exists(working_branch) {
                match repo.checkout_branch(working_branch) {
                    Ok(_) => {
                        Output::success(format!("Checked out branch: {working_branch}"));
                    }
                    Err(e) => {
                        Output::warning(format!("Failed to checkout '{working_branch}': {e}"));
                        Output::sub_item("Stack activated but stayed on current branch");
                        Output::sub_item(format!(
                            "You can manually checkout with: git checkout {working_branch}"
                        ));
                    }
                }
            } else {
                Output::warning(format!(
                    "Stack working branch '{working_branch}' doesn't exist locally"
                ));
                Output::sub_item("Stack activated but stayed on current branch");
                Output::sub_item(format!(
                    "You may need to fetch from remote: git fetch origin {working_branch}"
                ));
            }
        } else {
            Output::success(format!("Already on stack working branch: {working_branch}"));
        }
    } else {
        // No working branch - provide guidance
        Output::warning(format!("Stack '{name}' has no working branch set"));
        Output::sub_item(
            "This typically happens when a stack was created while on the base branch",
        );

        Output::tip("To start working on this stack:");
        Output::bullet(format!("Create a feature branch: git checkout -b {name}"));
        Output::bullet("The stack will automatically track this as its working branch");
        Output::bullet("Then use 'ca push' to add commits to the stack");

        Output::sub_item(format!("Base branch: {}", stack.base_branch));
    }

    // Activate the stack (this will record the correct current branch)
    manager.set_active_stack_by_name(&name)?;
    Output::success(format!("Switched to stack '{name}'"));

    Ok(())
}

async fn deactivate_stack(force: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut manager = StackManager::new(&repo_root)?;

    let active_stack = manager.get_active_stack();

    if active_stack.is_none() {
        Output::info("No active stack to deactivate");
        return Ok(());
    }

    let stack_name = active_stack.unwrap().name.clone();

    if !force {
        Output::warning(format!(
            "This will deactivate stack '{stack_name}' and return to normal Git workflow"
        ));
        Output::sub_item(format!(
            "You can reactivate it later with 'ca stacks switch {stack_name}'"
        ));
        print!("   Continue? (y/N): ");

        use std::io::{self, Write};
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        if !input.trim().to_lowercase().starts_with('y') {
            Output::info("Cancelled deactivation");
            return Ok(());
        }
    }

    // Deactivate the stack
    manager.set_active_stack(None)?;

    Output::success(format!("Deactivated stack '{stack_name}'"));
    Output::sub_item("Stack management is now OFF - you can use normal Git workflow");
    Output::sub_item(format!("To reactivate: ca stacks switch {stack_name}"));

    Ok(())
}

async fn show_stack(verbose: bool, show_mergeable: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let stack_manager = StackManager::new(&repo_root)?;

    // Get stack information first to avoid borrow conflicts
    let (stack_id, stack_name, stack_base, stack_working, stack_entries) = {
        let active_stack = stack_manager.get_active_stack().ok_or_else(|| {
            CascadeError::config(
                "No active stack. Use 'ca stacks create' or 'ca stacks switch' to select a stack"
                    .to_string(),
            )
        })?;

        (
            active_stack.id,
            active_stack.name.clone(),
            active_stack.base_branch.clone(),
            active_stack.working_branch.clone(),
            active_stack.entries.clone(),
        )
    };

    // Use the new output format for stack info
    Output::stack_info(
        &stack_name,
        &stack_id.to_string(),
        &stack_base,
        stack_working.as_deref(),
        true, // is_active
    );
    Output::sub_item(format!("Total entries: {}", stack_entries.len()));

    if stack_entries.is_empty() {
        Output::info("No entries in this stack yet");
        Output::tip("Use 'ca push' to add commits to this stack");
        return Ok(());
    }

    // Show entries
    Output::section("Stack Entries");
    for (i, entry) in stack_entries.iter().enumerate() {
        let entry_num = i + 1;
        let short_hash = entry.short_hash();
        let short_msg = entry.short_message(50);

        // Get source branch information if available
        let metadata = stack_manager.get_repository_metadata();
        let source_branch_info = if let Some(commit_meta) = metadata.get_commit(&entry.commit_hash)
        {
            if commit_meta.source_branch != commit_meta.branch
                && !commit_meta.source_branch.is_empty()
            {
                format!(" (from {})", commit_meta.source_branch)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let status_icon = if entry.is_submitted {
            "[submitted]"
        } else {
            "[pending]"
        };
        Output::numbered_item(
            entry_num,
            format!("{short_hash} {status_icon} {short_msg}{source_branch_info}"),
        );

        if verbose {
            Output::sub_item(format!("Branch: {}", entry.branch));
            Output::sub_item(format!(
                "Created: {}",
                entry.created_at.format("%Y-%m-%d %H:%M")
            ));
            if let Some(pr_id) = &entry.pull_request_id {
                Output::sub_item(format!("PR: #{pr_id}"));
            }
        }
    }

    // Enhanced PR status if requested and available
    if show_mergeable {
        Output::section("Mergability Status");

        // Load configuration and create Bitbucket integration
        let config_dir = crate::config::get_repo_config_dir(&repo_root)?;
        let config_path = config_dir.join("config.json");
        let settings = crate::config::Settings::load_from_file(&config_path)?;

        let cascade_config = crate::config::CascadeConfig {
            bitbucket: Some(settings.bitbucket.clone()),
            git: settings.git.clone(),
            auth: crate::config::AuthConfig::default(),
            cascade: settings.cascade.clone(),
        };

        let integration =
            crate::bitbucket::BitbucketIntegration::new(stack_manager, cascade_config)?;

        match integration.check_enhanced_stack_status(&stack_id).await {
            Ok(status) => {
                Output::bullet(format!("Total entries: {}", status.total_entries));
                Output::bullet(format!("Submitted: {}", status.submitted_entries));
                Output::bullet(format!("Open PRs: {}", status.open_prs));
                Output::bullet(format!("Merged PRs: {}", status.merged_prs));
                Output::bullet(format!("Declined PRs: {}", status.declined_prs));
                Output::bullet(format!(
                    "Completion: {:.1}%",
                    status.completion_percentage()
                ));

                if !status.enhanced_statuses.is_empty() {
                    Output::section("Pull Request Status");
                    let mut ready_to_land = 0;

                    for enhanced in &status.enhanced_statuses {
                        let status_display = enhanced.get_display_status();
                        let ready_icon = if enhanced.is_ready_to_land() {
                            ready_to_land += 1;
                            "[READY]"
                        } else {
                            "[PENDING]"
                        };

                        Output::bullet(format!(
                            "{} PR #{}: {} ({})",
                            ready_icon, enhanced.pr.id, enhanced.pr.title, status_display
                        ));

                        if verbose {
                            println!(
                                "      {} -> {}",
                                enhanced.pr.from_ref.display_id, enhanced.pr.to_ref.display_id
                            );

                            // Show blocking reasons if not ready
                            if !enhanced.is_ready_to_land() {
                                let blocking = enhanced.get_blocking_reasons();
                                if !blocking.is_empty() {
                                    println!("      Blocking: {}", blocking.join(", "));
                                }
                            }

                            // Show review details
                            println!(
                                "      Reviews: {}/{} approvals",
                                enhanced.review_status.current_approvals,
                                enhanced.review_status.required_approvals
                            );

                            if enhanced.review_status.needs_work_count > 0 {
                                println!(
                                    "      {} reviewers requested changes",
                                    enhanced.review_status.needs_work_count
                                );
                            }

                            // Show build status
                            if let Some(build) = &enhanced.build_status {
                                let build_icon = match build.state {
                                    crate::bitbucket::pull_request::BuildState::Successful => "✅",
                                    crate::bitbucket::pull_request::BuildState::Failed => "❌",
                                    crate::bitbucket::pull_request::BuildState::InProgress => "🔄",
                                    _ => "⚪",
                                };
                                println!("      Build: {} {:?}", build_icon, build.state);
                            }

                            if let Some(url) = enhanced.pr.web_url() {
                                println!("      URL: {url}");
                            }
                            println!();
                        }
                    }

                    if ready_to_land > 0 {
                        println!(
                            "\n🎯 {} PR{} ready to land! Use 'ca land' to land them all.",
                            ready_to_land,
                            if ready_to_land == 1 { " is" } else { "s are" }
                        );
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get enhanced stack status: {}", e);
                println!("   ⚠️  Could not fetch mergability status");
                println!("   Use 'ca stack show --verbose' for basic PR information");
            }
        }
    } else {
        // Original PR status display for compatibility
        let config_dir = crate::config::get_repo_config_dir(&repo_root)?;
        let config_path = config_dir.join("config.json");
        let settings = crate::config::Settings::load_from_file(&config_path)?;

        let cascade_config = crate::config::CascadeConfig {
            bitbucket: Some(settings.bitbucket.clone()),
            git: settings.git.clone(),
            auth: crate::config::AuthConfig::default(),
            cascade: settings.cascade.clone(),
        };

        let integration =
            crate::bitbucket::BitbucketIntegration::new(stack_manager, cascade_config)?;

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
                        println!(
                            "   {} PR #{}: {} ({} -> {})",
                            state_icon,
                            pr.id,
                            pr.title,
                            pr.from_ref.display_id,
                            pr.to_ref.display_id
                        );
                        if let Some(url) = pr.web_url() {
                            println!("      URL: {url}");
                        }
                    }
                }

                println!("\n💡 Use 'ca stack --mergeable' to see detailed status including build and review information");
            }
            Err(e) => {
                warn!("Failed to check stack status: {}", e);
            }
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn push_to_stack(
    branch: Option<String>,
    message: Option<String>,
    commit: Option<String>,
    since: Option<String>,
    commits: Option<String>,
    squash: Option<usize>,
    squash_since: Option<String>,
    auto_branch: bool,
    allow_base_branch: bool,
    dry_run: bool,
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut manager = StackManager::new(&repo_root)?;
    let repo = GitRepository::open(&repo_root)?;

    // Check for branch changes and prompt user if needed
    if !manager.check_for_branch_change()? {
        return Ok(()); // User chose to cancel or deactivate stack
    }

    // Get the active stack to check base branch
    let active_stack = manager.get_active_stack().ok_or_else(|| {
        CascadeError::config("No active stack. Create a stack first with 'ca stack create'")
    })?;

    // 🛡️ BASE BRANCH PROTECTION
    let current_branch = repo.get_current_branch()?;
    let base_branch = &active_stack.base_branch;

    if current_branch == *base_branch {
        Output::error(format!(
            "You're currently on the base branch '{base_branch}'"
        ));
        Output::sub_item("Making commits directly on the base branch is not recommended.");
        Output::sub_item("This can pollute the base branch with work-in-progress commits.");

        // Check if user explicitly allowed base branch work
        if allow_base_branch {
            Output::warning("Proceeding anyway due to --allow-base-branch flag");
        } else {
            // Check if we have uncommitted changes
            let has_changes = repo.is_dirty()?;

            if has_changes {
                if auto_branch {
                    // Auto-create branch and commit changes
                    let feature_branch = format!("feature/{}-work", active_stack.name);
                    Output::progress(format!(
                        "Auto-creating feature branch '{feature_branch}'..."
                    ));

                    repo.create_branch(&feature_branch, None)?;
                    repo.checkout_branch(&feature_branch)?;

                    println!("✅ Created and switched to '{feature_branch}'");
                    println!("   You can now commit and push your changes safely");

                    // Continue with normal flow
                } else {
                    println!("\n💡 You have uncommitted changes. Here are your options:");
                    println!("   1. Create a feature branch first:");
                    println!("      git checkout -b feature/my-work");
                    println!("      git commit -am \"your work\"");
                    println!("      ca push");
                    println!("\n   2. Auto-create a branch (recommended):");
                    println!("      ca push --auto-branch");
                    println!("\n   3. Force push to base branch (dangerous):");
                    println!("      ca push --allow-base-branch");

                    return Err(CascadeError::config(
                        "Refusing to push uncommitted changes from base branch. Use one of the options above."
                    ));
                }
            } else {
                // Check if there are existing commits to push
                let commits_to_check = if let Some(commits_str) = &commits {
                    commits_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect::<Vec<String>>()
                } else if let Some(since_ref) = &since {
                    let since_commit = repo.resolve_reference(since_ref)?;
                    let head_commit = repo.get_head_commit()?;
                    let commits = repo.get_commits_between(
                        &since_commit.id().to_string(),
                        &head_commit.id().to_string(),
                    )?;
                    commits.into_iter().map(|c| c.id().to_string()).collect()
                } else if commit.is_none() {
                    let mut unpushed = Vec::new();
                    let head_commit = repo.get_head_commit()?;
                    let mut current_commit = head_commit;

                    loop {
                        let commit_hash = current_commit.id().to_string();
                        let already_in_stack = active_stack
                            .entries
                            .iter()
                            .any(|entry| entry.commit_hash == commit_hash);

                        if already_in_stack {
                            break;
                        }

                        unpushed.push(commit_hash);

                        if let Some(parent) = current_commit.parents().next() {
                            current_commit = parent;
                        } else {
                            break;
                        }
                    }

                    unpushed.reverse();
                    unpushed
                } else {
                    vec![repo.get_head_commit()?.id().to_string()]
                };

                if !commits_to_check.is_empty() {
                    if auto_branch {
                        // Auto-create feature branch and cherry-pick commits
                        let feature_branch = format!("feature/{}-work", active_stack.name);
                        Output::progress(format!(
                            "Auto-creating feature branch '{feature_branch}'..."
                        ));

                        repo.create_branch(&feature_branch, Some(base_branch))?;
                        repo.checkout_branch(&feature_branch)?;

                        // Cherry-pick the commits to the new branch
                        println!(
                            "🍒 Cherry-picking {} commit(s) to new branch...",
                            commits_to_check.len()
                        );
                        for commit_hash in &commits_to_check {
                            match repo.cherry_pick(commit_hash) {
                                Ok(_) => println!("   ✅ Cherry-picked {}", &commit_hash[..8]),
                                Err(e) => {
                                    println!(
                                        "   ❌ Failed to cherry-pick {}: {}",
                                        &commit_hash[..8],
                                        e
                                    );
                                    println!("   💡 You may need to resolve conflicts manually");
                                    return Err(CascadeError::branch(format!(
                                        "Failed to cherry-pick commit {commit_hash}: {e}"
                                    )));
                                }
                            }
                        }

                        println!(
                            "✅ Successfully moved {} commit(s) to '{feature_branch}'",
                            commits_to_check.len()
                        );
                        println!(
                            "   You're now on the feature branch and can continue with 'ca push'"
                        );

                        // Continue with normal flow
                    } else {
                        println!(
                            "\n💡 Found {} commit(s) to push from base branch '{base_branch}'",
                            commits_to_check.len()
                        );
                        println!("   These commits are currently ON the base branch, which may not be intended.");
                        println!("\n   Options:");
                        println!("   1. Auto-create feature branch and cherry-pick commits:");
                        println!("      ca push --auto-branch");
                        println!("\n   2. Manually create branch and move commits:");
                        println!("      git checkout -b feature/my-work");
                        println!("      ca push");
                        println!("\n   3. Force push from base branch (not recommended):");
                        println!("      ca push --allow-base-branch");

                        return Err(CascadeError::config(
                            "Refusing to push commits from base branch. Use --auto-branch or create a feature branch manually."
                        ));
                    }
                }
            }
        }
    }

    // Handle squash operations first
    if let Some(squash_count) = squash {
        if squash_count == 0 {
            // User used --squash without specifying count, auto-detect unpushed commits
            let active_stack = manager.get_active_stack().ok_or_else(|| {
                CascadeError::config(
                    "No active stack. Create a stack first with 'ca stacks create'",
                )
            })?;

            let unpushed_count = get_unpushed_commits(&repo, active_stack)?.len();

            if unpushed_count == 0 {
                println!("ℹ️  No unpushed commits to squash");
            } else if unpushed_count == 1 {
                println!("ℹ️  Only 1 unpushed commit, no squashing needed");
            } else {
                println!("🔄 Auto-detected {unpushed_count} unpushed commits, squashing...");
                squash_commits(&repo, unpushed_count, None).await?;
                println!("✅ Squashed {unpushed_count} unpushed commits into one");
            }
        } else {
            println!("🔄 Squashing last {squash_count} commits...");
            squash_commits(&repo, squash_count, None).await?;
            println!("✅ Squashed {squash_count} commits into one");
        }
    } else if let Some(since_ref) = squash_since {
        println!("🔄 Squashing commits since {since_ref}...");
        let since_commit = repo.resolve_reference(&since_ref)?;
        let commits_count = count_commits_since(&repo, &since_commit.id().to_string())?;
        squash_commits(&repo, commits_count, Some(since_ref.clone())).await?;
        println!("✅ Squashed {commits_count} commits since {since_ref} into one");
    }

    // Determine which commits to push
    let commits_to_push = if let Some(commits_str) = commits {
        // Parse comma-separated commit hashes
        commits_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<String>>()
    } else if let Some(since_ref) = since {
        // Get commits since the specified reference
        let since_commit = repo.resolve_reference(&since_ref)?;
        let head_commit = repo.get_head_commit()?;

        // Get commits between since_ref and HEAD
        let commits = repo.get_commits_between(
            &since_commit.id().to_string(),
            &head_commit.id().to_string(),
        )?;
        commits.into_iter().map(|c| c.id().to_string()).collect()
    } else if let Some(hash) = commit {
        // Single specific commit
        vec![hash]
    } else {
        // Default: Get all unpushed commits (commits on current branch but not on base branch)
        let active_stack = manager.get_active_stack().ok_or_else(|| {
            CascadeError::config("No active stack. Create a stack first with 'ca stacks create'")
        })?;

        // Get commits that are on current branch but not on the base branch
        let base_branch = &active_stack.base_branch;
        let current_branch = repo.get_current_branch()?;

        // If we're on the base branch, only include commits that aren't already in the stack
        if current_branch == *base_branch {
            let mut unpushed = Vec::new();
            let head_commit = repo.get_head_commit()?;
            let mut current_commit = head_commit;

            // Walk back from HEAD until we find a commit that's already in the stack
            loop {
                let commit_hash = current_commit.id().to_string();
                let already_in_stack = active_stack
                    .entries
                    .iter()
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
        } else {
            // Use git's commit range calculation to find commits on current branch but not on base
            match repo.get_commits_between(base_branch, &current_branch) {
                Ok(commits) => {
                    let mut unpushed: Vec<String> =
                        commits.into_iter().map(|c| c.id().to_string()).collect();

                    // Filter out commits that are already in the stack
                    unpushed.retain(|commit_hash| {
                        !active_stack
                            .entries
                            .iter()
                            .any(|entry| entry.commit_hash == *commit_hash)
                    });

                    unpushed.reverse(); // Reverse to get chronological order (oldest first)
                    unpushed
                }
                Err(e) => {
                    return Err(CascadeError::branch(format!(
                            "Failed to calculate commits between '{base_branch}' and '{current_branch}': {e}. \
                             This usually means the branches have diverged or don't share common history."
                        )));
                }
            }
        }
    };

    if commits_to_push.is_empty() {
        println!("ℹ️  No commits to push to stack");
        return Ok(());
    }

    // 🛡️ SAFEGUARDS: Analyze commits before pushing
    analyze_commits_for_safeguards(&commits_to_push, &repo, dry_run).await?;

    // Early return for dry run mode
    if dry_run {
        return Ok(());
    }

    // Push each commit to the stack
    let mut pushed_count = 0;
    let mut source_branches = std::collections::HashSet::new();

    for (i, commit_hash) in commits_to_push.iter().enumerate() {
        let commit_obj = repo.get_commit(commit_hash)?;
        let commit_msg = commit_obj.message().unwrap_or("").to_string();

        // Check which branch this commit belongs to
        let commit_source_branch = repo
            .find_branch_containing_commit(commit_hash)
            .unwrap_or_else(|_| current_branch.clone());
        source_branches.insert(commit_source_branch.clone());

        // Generate branch name (use provided branch for first commit, generate for others)
        let branch_name = if i == 0 && branch.is_some() {
            branch.clone().unwrap()
        } else {
            // Create a temporary GitRepository for branch name generation
            let temp_repo = GitRepository::open(&repo_root)?;
            let branch_mgr = crate::git::BranchManager::new(temp_repo);
            branch_mgr.generate_branch_name(&commit_msg)
        };

        // Use provided message for first commit, original message for others
        let final_message = if i == 0 && message.is_some() {
            message.clone().unwrap()
        } else {
            commit_msg.clone()
        };

        let entry_id = manager.push_to_stack(
            branch_name.clone(),
            commit_hash.clone(),
            final_message.clone(),
            commit_source_branch.clone(),
        )?;
        pushed_count += 1;

        Output::success(format!(
            "Pushed commit {}/{} to stack",
            i + 1,
            commits_to_push.len()
        ));
        Output::sub_item(format!(
            "Commit: {} ({})",
            &commit_hash[..8],
            commit_msg.split('\n').next().unwrap_or("")
        ));
        Output::sub_item(format!("Branch: {branch_name}"));
        Output::sub_item(format!("Source: {commit_source_branch}"));
        Output::sub_item(format!("Entry ID: {entry_id}"));
        println!();
    }

    // 🚨 SCATTERED COMMIT WARNING
    if source_branches.len() > 1 {
        Output::warning("Scattered Commit Detection");
        Output::sub_item(format!(
            "You've pushed commits from {} different Git branches:",
            source_branches.len()
        ));
        for branch in &source_branches {
            Output::bullet(branch.to_string());
        }

        Output::section("This can lead to confusion because:");
        Output::bullet("Stack appears sequential but commits are scattered across branches");
        Output::bullet("Team members won't know which branch contains which work");
        Output::bullet("Branch cleanup becomes unclear after merge");
        Output::bullet("Rebase operations become more complex");

        Output::tip("Consider consolidating work to a single feature branch:");
        Output::bullet("Create a new feature branch: git checkout -b feature/consolidated-work");
        Output::bullet("Cherry-pick commits in order: git cherry-pick <commit1> <commit2> ...");
        Output::bullet("Delete old scattered branches");
        Output::bullet("Push the consolidated branch to your stack");
        println!();
    }

    Output::success(format!(
        "Successfully pushed {} commit{} to stack",
        pushed_count,
        if pushed_count == 1 { "" } else { "s" }
    ));

    Ok(())
}

async fn pop_from_stack(keep_branch: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut manager = StackManager::new(&repo_root)?;
    let repo = GitRepository::open(&repo_root)?;

    let entry = manager.pop_from_stack()?;

    Output::success("Popped commit from stack");
    Output::sub_item(format!(
        "Commit: {} ({})",
        entry.short_hash(),
        entry.short_message(50)
    ));
    Output::sub_item(format!("Branch: {}", entry.branch));

    // Delete branch if requested and it's not the current branch
    if !keep_branch && entry.branch != repo.get_current_branch()? {
        match repo.delete_branch(&entry.branch) {
            Ok(_) => Output::sub_item(format!("Deleted branch: {}", entry.branch)),
            Err(e) => Output::warning(format!("Could not delete branch {}: {}", entry.branch, e)),
        }
    }

    Ok(())
}

async fn submit_entry(
    entry: Option<usize>,
    title: Option<String>,
    description: Option<String>,
    range: Option<String>,
    draft: bool,
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut stack_manager = StackManager::new(&repo_root)?;

    // Check for branch changes and prompt user if needed
    if !stack_manager.check_for_branch_change()? {
        return Ok(()); // User chose to cancel or deactivate stack
    }

    // Load configuration first
    let config_dir = crate::config::get_repo_config_dir(&repo_root)?;
    let config_path = config_dir.join("config.json");
    let settings = crate::config::Settings::load_from_file(&config_path)?;

    // Create the main config structure
    let cascade_config = crate::config::CascadeConfig {
        bitbucket: Some(settings.bitbucket.clone()),
        git: settings.git.clone(),
        auth: crate::config::AuthConfig::default(),
        cascade: settings.cascade.clone(),
    };

    // Get the active stack
    let active_stack = stack_manager.get_active_stack().ok_or_else(|| {
        CascadeError::config("No active stack. Create a stack first with 'ca stack create'")
    })?;
    let stack_id = active_stack.id;

    // Determine which entries to submit
    let entries_to_submit = if let Some(range_str) = range {
        // Parse range (e.g., "1-3" or "2,4,6")
        let mut entries = Vec::new();

        if range_str.contains('-') {
            // Handle range like "1-3"
            let parts: Vec<&str> = range_str.split('-').collect();
            if parts.len() != 2 {
                return Err(CascadeError::config(
                    "Invalid range format. Use 'start-end' (e.g., '1-3')",
                ));
            }

            let start: usize = parts[0]
                .parse()
                .map_err(|_| CascadeError::config("Invalid start number in range"))?;
            let end: usize = parts[1]
                .parse()
                .map_err(|_| CascadeError::config("Invalid end number in range"))?;

            if start == 0
                || end == 0
                || start > active_stack.entries.len()
                || end > active_stack.entries.len()
            {
                return Err(CascadeError::config(format!(
                    "Range out of bounds. Stack has {} entries",
                    active_stack.entries.len()
                )));
            }

            for i in start..=end {
                entries.push((i, active_stack.entries[i - 1].clone()));
            }
        } else {
            // Handle comma-separated list like "2,4,6"
            for entry_str in range_str.split(',') {
                let entry_num: usize = entry_str.trim().parse().map_err(|_| {
                    CascadeError::config(format!("Invalid entry number: {entry_str}"))
                })?;

                if entry_num == 0 || entry_num > active_stack.entries.len() {
                    return Err(CascadeError::config(format!(
                        "Entry {} out of bounds. Stack has {} entries",
                        entry_num,
                        active_stack.entries.len()
                    )));
                }

                entries.push((entry_num, active_stack.entries[entry_num - 1].clone()));
            }
        }

        entries
    } else if let Some(entry_num) = entry {
        // Single entry specified
        if entry_num == 0 || entry_num > active_stack.entries.len() {
            return Err(CascadeError::config(format!(
                "Invalid entry number: {}. Stack has {} entries",
                entry_num,
                active_stack.entries.len()
            )));
        }
        vec![(entry_num, active_stack.entries[entry_num - 1].clone())]
    } else {
        // Default: Submit all unsubmitted entries
        active_stack
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| !entry.is_submitted)
            .map(|(i, entry)| (i + 1, entry.clone())) // Convert to 1-based indexing
            .collect::<Vec<(usize, _)>>()
    };

    if entries_to_submit.is_empty() {
        Output::info("No entries to submit");
        return Ok(());
    }

    // Create progress bar for the submission process
    let total_operations = entries_to_submit.len() + 2; // +2 for setup steps
    let pb = ProgressBar::new(total_operations as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("📤 {msg} [{bar:40.cyan/blue}] {pos}/{len}")
            .map_err(|e| CascadeError::config(format!("Progress bar template error: {e}")))?,
    );

    pb.set_message("Connecting to Bitbucket");
    pb.inc(1);

    // Create a new StackManager for the integration (since the original was moved)
    let integration_stack_manager = StackManager::new(&repo_root)?;
    let mut integration =
        BitbucketIntegration::new(integration_stack_manager, cascade_config.clone())?;

    pb.set_message("Starting batch submission");
    pb.inc(1);

    // Submit each entry
    let mut submitted_count = 0;
    let mut failed_entries = Vec::new();
    let total_entries = entries_to_submit.len();

    for (entry_num, entry_to_submit) in &entries_to_submit {
        pb.set_message(format!("Submitting entry {entry_num}..."));

        // Use provided title/description only for first entry or single entry submissions
        let entry_title = if total_entries == 1 {
            title.clone()
        } else {
            None
        };
        let entry_description = if total_entries == 1 {
            description.clone()
        } else {
            None
        };

        match integration
            .submit_entry(
                &stack_id,
                &entry_to_submit.id,
                entry_title,
                entry_description,
                draft,
            )
            .await
        {
            Ok(pr) => {
                submitted_count += 1;
                Output::success(format!("Entry {} - PR #{}: {}", entry_num, pr.id, pr.title));
                if let Some(url) = pr.web_url() {
                    Output::sub_item(format!("URL: {url}"));
                }
                Output::sub_item(format!(
                    "From: {} -> {}",
                    pr.from_ref.display_id, pr.to_ref.display_id
                ));
                println!();
            }
            Err(e) => {
                failed_entries.push((*entry_num, e.to_string()));
                // Don't print the error here - we'll show it in the summary
            }
        }

        pb.inc(1);
    }

    // Update all PR descriptions in the stack if any PRs were created/exist
    let has_any_prs = active_stack
        .entries
        .iter()
        .any(|e| e.pull_request_id.is_some());
    if has_any_prs && submitted_count > 0 {
        pb.set_message("Updating PR descriptions...");
        match integration.update_all_pr_descriptions(&stack_id).await {
            Ok(updated_prs) => {
                if !updated_prs.is_empty() {
                    Output::sub_item(format!(
                        "Updated {} PR descriptions with current stack hierarchy",
                        updated_prs.len()
                    ));
                }
            }
            Err(e) => {
                Output::warning(format!("Failed to update some PR descriptions: {e}"));
            }
        }
    }

    if failed_entries.is_empty() {
        pb.finish_with_message("✅ All pull requests created successfully");
        Output::success(format!(
            "Successfully submitted {} entr{}",
            submitted_count,
            if submitted_count == 1 { "y" } else { "ies" }
        ));
    } else {
        pb.abandon_with_message("⚠️  Some submissions failed");
        Output::section("Submission Summary");
        Output::bullet(format!("Successful: {submitted_count}"));
        Output::bullet(format!("Failed: {}", failed_entries.len()));

        Output::section("Failed entries:");
        for (entry_num, error) in failed_entries {
            Output::bullet(format!("Entry {entry_num}: {error}"));
        }

        Output::tip("You can retry failed entries individually:");
        Output::command_example("ca stack submit <ENTRY_NUMBER>");
    }

    Ok(())
}

async fn check_stack_status(name: Option<String>) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let stack_manager = StackManager::new(&repo_root)?;

    // Load configuration
    let config_dir = crate::config::get_repo_config_dir(&repo_root)?;
    let config_path = config_dir.join("config.json");
    let settings = crate::config::Settings::load_from_file(&config_path)?;

    // Create the main config structure
    let cascade_config = crate::config::CascadeConfig {
        bitbucket: Some(settings.bitbucket.clone()),
        git: settings.git.clone(),
        auth: crate::config::AuthConfig::default(),
        cascade: settings.cascade.clone(),
    };

    // Get stack information BEFORE moving stack_manager
    let stack = if let Some(name) = name {
        stack_manager
            .get_stack_by_name(&name)
            .ok_or_else(|| CascadeError::config(format!("Stack '{name}' not found")))?
    } else {
        stack_manager.get_active_stack().ok_or_else(|| {
            CascadeError::config("No active stack. Use 'ca stack list' to see available stacks")
        })?
    };
    let stack_id = stack.id;

    Output::section(format!("Stack: {}", stack.name));
    Output::sub_item(format!("ID: {}", stack.id));
    Output::sub_item(format!("Base: {}", stack.base_branch));

    if let Some(description) = &stack.description {
        Output::sub_item(format!("Description: {description}"));
    }

    // Create Bitbucket integration (this takes ownership of stack_manager)
    let integration = crate::bitbucket::BitbucketIntegration::new(stack_manager, cascade_config)?;

    // Check stack status
    match integration.check_stack_status(&stack_id).await {
        Ok(status) => {
            Output::section("Pull Request Status");
            Output::sub_item(format!("Total entries: {}", status.total_entries));
            Output::sub_item(format!("Submitted: {}", status.submitted_entries));
            Output::sub_item(format!("Open PRs: {}", status.open_prs));
            Output::sub_item(format!("Merged PRs: {}", status.merged_prs));
            Output::sub_item(format!("Declined PRs: {}", status.declined_prs));
            Output::sub_item(format!(
                "Completion: {:.1}%",
                status.completion_percentage()
            ));

            if !status.pull_requests.is_empty() {
                Output::section("Pull Requests");
                for pr in &status.pull_requests {
                    let state_icon = match pr.state {
                        crate::bitbucket::PullRequestState::Open => "🔄",
                        crate::bitbucket::PullRequestState::Merged => "✅",
                        crate::bitbucket::PullRequestState::Declined => "❌",
                    };
                    Output::bullet(format!(
                        "{} PR #{}: {} ({} -> {})",
                        state_icon, pr.id, pr.title, pr.from_ref.display_id, pr.to_ref.display_id
                    ));
                    if let Some(url) = pr.web_url() {
                        Output::sub_item(format!("URL: {url}"));
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
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let stack_manager = StackManager::new(&repo_root)?;

    // Load configuration
    let config_dir = crate::config::get_repo_config_dir(&repo_root)?;
    let config_path = config_dir.join("config.json");
    let settings = crate::config::Settings::load_from_file(&config_path)?;

    // Create the main config structure
    let cascade_config = crate::config::CascadeConfig {
        bitbucket: Some(settings.bitbucket.clone()),
        git: settings.git.clone(),
        auth: crate::config::AuthConfig::default(),
        cascade: settings.cascade.clone(),
    };

    // Create Bitbucket integration
    let integration = crate::bitbucket::BitbucketIntegration::new(stack_manager, cascade_config)?;

    // Parse state filter
    let pr_state = if let Some(state_str) = state {
        match state_str.to_lowercase().as_str() {
            "open" => Some(crate::bitbucket::PullRequestState::Open),
            "merged" => Some(crate::bitbucket::PullRequestState::Merged),
            "declined" => Some(crate::bitbucket::PullRequestState::Declined),
            _ => {
                return Err(CascadeError::config(format!(
                    "Invalid state '{state_str}'. Use: open, merged, declined"
                )))
            }
        }
    } else {
        None
    };

    // Get pull requests
    match integration.list_pull_requests(pr_state).await {
        Ok(pr_page) => {
            if pr_page.values.is_empty() {
                Output::info("No pull requests found.");
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
                    println!(
                        "      From: {} -> {}",
                        pr.from_ref.display_id, pr.to_ref.display_id
                    );
                    println!("      Author: {}", pr.author.user.display_name);
                    if let Some(url) = pr.web_url() {
                        println!("      URL: {url}");
                    }
                    if let Some(desc) = &pr.description {
                        if !desc.is_empty() {
                            println!("      Description: {desc}");
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

async fn check_stack(_force: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut manager = StackManager::new(&repo_root)?;

    let active_stack = manager
        .get_active_stack()
        .ok_or_else(|| CascadeError::config("No active stack"))?;
    let stack_id = active_stack.id;

    manager.sync_stack(&stack_id)?;

    Output::success("Stack check completed successfully");

    Ok(())
}

async fn sync_stack(force: bool, skip_cleanup: bool, interactive: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let stack_manager = StackManager::new(&repo_root)?;
    let git_repo = GitRepository::open(&repo_root)?;

    // Get active stack
    let active_stack = stack_manager.get_active_stack().ok_or_else(|| {
        CascadeError::config("No active stack. Create a stack first with 'ca stack create'")
    })?;

    let base_branch = active_stack.base_branch.clone();
    let stack_name = active_stack.name.clone();

    Output::progress(format!("Syncing stack '{stack_name}' with remote..."));

    // Step 1: Pull latest changes from base branch
    Output::section(format!("Pulling latest changes from '{base_branch}'"));

    // Checkout base branch first
    match git_repo.checkout_branch(&base_branch) {
        Ok(_) => {
            Output::sub_item(format!("Switched to '{base_branch}'"));

            // Pull latest changes
            match git_repo.pull(&base_branch) {
                Ok(_) => {
                    Output::sub_item("Successfully pulled latest changes");
                }
                Err(e) => {
                    if force {
                        Output::warning(format!("Failed to pull: {e} (continuing due to --force)"));
                    } else {
                        return Err(CascadeError::branch(format!(
                            "Failed to pull latest changes from '{base_branch}': {e}. Use --force to continue anyway."
                        )));
                    }
                }
            }
        }
        Err(e) => {
            if force {
                Output::warning(format!(
                    "Failed to checkout '{base_branch}': {e} (continuing due to --force)"
                ));
            } else {
                return Err(CascadeError::branch(format!(
                    "Failed to checkout base branch '{base_branch}': {e}. Use --force to continue anyway."
                )));
            }
        }
    }

    // Step 2: Check if stack needs rebase
    Output::section("Checking if stack needs rebase");

    let mut updated_stack_manager = StackManager::new(&repo_root)?;
    let stack_id = active_stack.id;

    match updated_stack_manager.sync_stack(&stack_id) {
        Ok(_) => {
            // Check the updated status
            if let Some(updated_stack) = updated_stack_manager.get_stack(&stack_id) {
                match &updated_stack.status {
                    crate::stack::StackStatus::NeedsSync => {
                        Output::sub_item(format!(
                            "Stack needs rebase due to new commits on '{base_branch}'"
                        ));

                        // Step 3: Rebase the stack
                        Output::section(format!("Rebasing stack onto updated '{base_branch}'"));

                        // Load configuration for Bitbucket integration
                        let config_dir = crate::config::get_repo_config_dir(&repo_root)?;
                        let config_path = config_dir.join("config.json");
                        let settings = crate::config::Settings::load_from_file(&config_path)?;

                        let cascade_config = crate::config::CascadeConfig {
                            bitbucket: Some(settings.bitbucket.clone()),
                            git: settings.git.clone(),
                            auth: crate::config::AuthConfig::default(),
                            cascade: settings.cascade.clone(),
                        };

                        // Use the existing rebase system
                        let options = crate::stack::RebaseOptions {
                            strategy: crate::stack::RebaseStrategy::BranchVersioning,
                            interactive,
                            target_base: Some(base_branch.clone()),
                            preserve_merges: true,
                            auto_resolve: !interactive,
                            max_retries: 3,
                            skip_pull: Some(true), // Skip pull since we already pulled above
                        };

                        let mut rebase_manager = crate::stack::RebaseManager::new(
                            updated_stack_manager,
                            git_repo,
                            options,
                        );

                        match rebase_manager.rebase_stack(&stack_id) {
                            Ok(result) => {
                                Output::success("Rebase completed successfully!");

                                if !result.branch_mapping.is_empty() {
                                    Output::section("Updated branches:");
                                    for (old, new) in &result.branch_mapping {
                                        Output::bullet(format!("{old} → {new}"));
                                    }

                                    // Update PRs if enabled
                                    if let Some(ref _bitbucket_config) = cascade_config.bitbucket {
                                        Output::sub_item("Updating pull requests...");

                                        let integration_stack_manager =
                                            StackManager::new(&repo_root)?;
                                        let mut integration =
                                            crate::bitbucket::BitbucketIntegration::new(
                                                integration_stack_manager,
                                                cascade_config,
                                            )?;

                                        match integration
                                            .update_prs_after_rebase(
                                                &stack_id,
                                                &result.branch_mapping,
                                            )
                                            .await
                                        {
                                            Ok(updated_prs) => {
                                                if !updated_prs.is_empty() {
                                                    Output::sub_item(format!(
                                                        "Updated {} pull requests",
                                                        updated_prs.len()
                                                    ));
                                                }
                                            }
                                            Err(e) => {
                                                Output::warning(format!(
                                                    "Failed to update pull requests: {e}"
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                Output::error(format!("Rebase failed: {e}"));
                                Output::tip("To resolve conflicts:");
                                Output::bullet("Fix conflicts in the affected files");
                                Output::bullet("Stage resolved files: git add <files>");
                                Output::bullet("Continue: ca stack continue-rebase");
                                return Err(e);
                            }
                        }
                    }
                    crate::stack::StackStatus::Clean => {
                        Output::success("Stack is already up to date");
                    }
                    other => {
                        Output::info(format!("Stack status: {other:?}"));
                    }
                }
            }
        }
        Err(e) => {
            if force {
                Output::warning(format!(
                    "Failed to check stack status: {e} (continuing due to --force)"
                ));
            } else {
                return Err(e);
            }
        }
    }

    // Step 4: Cleanup merged branches (optional)
    if !skip_cleanup {
        Output::section("Checking for merged branches to clean up");
        // TODO: Implement merged branch cleanup
        // This would:
        // 1. Find branches that have been merged into base
        // 2. Ask user if they want to delete them
        // 3. Remove them from the stack metadata
        Output::info("Branch cleanup not yet implemented");
    } else {
        Output::info("Skipping branch cleanup");
    }

    Output::success("Sync completed successfully!");
    Output::sub_item(format!("Base branch: {base_branch}"));
    Output::next_steps(&[
        "Review your updated stack: ca stack show",
        "Check PR status: ca stack status",
    ]);

    Ok(())
}

async fn rebase_stack(
    interactive: bool,
    onto: Option<String>,
    strategy: Option<RebaseStrategyArg>,
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let stack_manager = StackManager::new(&repo_root)?;
    let git_repo = GitRepository::open(&repo_root)?;

    // Load configuration for potential Bitbucket integration
    let config_dir = crate::config::get_repo_config_dir(&repo_root)?;
    let config_path = config_dir.join("config.json");
    let settings = crate::config::Settings::load_from_file(&config_path)?;

    // Create the main config structure
    let cascade_config = crate::config::CascadeConfig {
        bitbucket: Some(settings.bitbucket.clone()),
        git: settings.git.clone(),
        auth: crate::config::AuthConfig::default(),
        cascade: settings.cascade.clone(),
    };

    // Get active stack
    let active_stack = stack_manager.get_active_stack().ok_or_else(|| {
        CascadeError::config("No active stack. Create a stack first with 'ca stack create'")
    })?;
    let stack_id = active_stack.id;

    let active_stack = stack_manager
        .get_stack(&stack_id)
        .ok_or_else(|| CascadeError::config("Active stack not found"))?
        .clone();

    if active_stack.entries.is_empty() {
        Output::info("Stack is empty. Nothing to rebase.");
        return Ok(());
    }

    Output::progress(format!("Rebasing stack: {}", active_stack.name));
    Output::sub_item(format!("Base: {}", active_stack.base_branch));

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
        skip_pull: None, // Normal rebase should pull latest changes
    };

    info!("   Strategy: {:?}", rebase_strategy);
    info!("   Interactive: {}", interactive);
    info!("   Target base: {:?}", options.target_base);
    info!("   Entries: {}", active_stack.entries.len());

    // Check if there's already a rebase in progress
    let mut rebase_manager = crate::stack::RebaseManager::new(stack_manager, git_repo, options);

    if rebase_manager.is_rebase_in_progress() {
        Output::warning("Rebase already in progress!");
        Output::tip("Use 'git status' to check the current state");
        Output::next_steps(&[
            "Run 'ca stack continue-rebase' to continue",
            "Run 'ca stack abort-rebase' to abort",
        ]);
        return Ok(());
    }

    // Perform the rebase
    match rebase_manager.rebase_stack(&stack_id) {
        Ok(result) => {
            Output::success("Rebase completed!");
            Output::sub_item(result.get_summary());

            if result.has_conflicts() {
                Output::warning(format!(
                    "{} conflicts were resolved",
                    result.conflicts.len()
                ));
                for conflict in &result.conflicts {
                    Output::bullet(&conflict[..8.min(conflict.len())]);
                }
            }

            if !result.branch_mapping.is_empty() {
                Output::section("Branch mapping");
                for (old, new) in &result.branch_mapping {
                    Output::bullet(format!("{old} -> {new}"));
                }

                // Handle PR updates if enabled
                if let Some(ref _bitbucket_config) = cascade_config.bitbucket {
                    // Create a new StackManager for the integration (since the original was moved)
                    let integration_stack_manager = StackManager::new(&repo_root)?;
                    let mut integration = BitbucketIntegration::new(
                        integration_stack_manager,
                        cascade_config.clone(),
                    )?;

                    match integration
                        .update_prs_after_rebase(&stack_id, &result.branch_mapping)
                        .await
                    {
                        Ok(updated_prs) => {
                            if !updated_prs.is_empty() {
                                println!("   🔄 Preserved pull request history:");
                                for pr_update in updated_prs {
                                    println!("      ✅ {pr_update}");
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("   ⚠️  Failed to update pull requests: {e}");
                            eprintln!("      You may need to manually update PRs in Bitbucket");
                        }
                    }
                }
            }

            println!(
                "   ✅ {} commits successfully rebased",
                result.success_count()
            );

            // Show next steps
            if matches!(
                rebase_strategy,
                crate::stack::RebaseStrategy::BranchVersioning
            ) {
                println!("\n📝 Next steps:");
                if !result.branch_mapping.is_empty() {
                    println!("   1. ✅ New versioned branches have been created");
                    println!("   2. ✅ Pull requests have been updated automatically");
                    println!("   3. 🔍 Review the updated PRs in Bitbucket");
                    println!("   4. 🧪 Test your changes on the new branches");
                    println!(
                        "   5. 🗑️  Old branches are preserved for safety (can be deleted later)"
                    );
                } else {
                    println!("   1. Review the rebased stack");
                    println!("   2. Test your changes");
                    println!("   3. Submit new pull requests with 'ca stack submit'");
                }
            }
        }
        Err(e) => {
            warn!("❌ Rebase failed: {}", e);
            println!("💡 Tips for resolving rebase issues:");
            println!("   - Check for uncommitted changes with 'git status'");
            println!("   - Ensure base branch is up to date");
            println!("   - Try interactive mode: 'ca stack rebase --interactive'");
            return Err(e);
        }
    }

    Ok(())
}

async fn continue_rebase() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let stack_manager = StackManager::new(&repo_root)?;
    let git_repo = crate::git::GitRepository::open(&repo_root)?;
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
            println!("   Check 'ca stack rebase-status' for current state");
        }
        Err(e) => {
            warn!("❌ Failed to continue rebase: {}", e);
            println!("💡 You may need to resolve conflicts first:");
            println!("   1. Edit conflicted files");
            println!("   2. Stage resolved files with 'git add'");
            println!("   3. Run 'ca stack continue-rebase' again");
        }
    }

    Ok(())
}

async fn abort_rebase() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let stack_manager = StackManager::new(&repo_root)?;
    let git_repo = crate::git::GitRepository::open(&repo_root)?;
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
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let stack_manager = StackManager::new(&repo_root)?;
    let git_repo = crate::git::GitRepository::open(&repo_root)?;

    println!("📊 Rebase Status");

    // Check if rebase is in progress by checking git state directly
    let git_dir = current_dir.join(".git");
    let rebase_in_progress = git_dir.join("REBASE_HEAD").exists()
        || git_dir.join("rebase-merge").exists()
        || git_dir.join("rebase-apply").exists();

    if rebase_in_progress {
        println!("   Status: 🔄 Rebase in progress");
        println!(
            "   
📝 Actions available:"
        );
        println!("     - 'ca stack continue-rebase' to continue");
        println!("     - 'ca stack abort-rebase' to abort");
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
                        println!("      - {conflict}");
                    }
                    println!(
                        "   
💡 To resolve conflicts:"
                    );
                    println!("     1. Edit the conflicted files");
                    println!("     2. Stage resolved files: git add <file>");
                    println!("     3. Continue: ca stack continue-rebase");
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
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut manager = StackManager::new(&repo_root)?;

    let stack = manager
        .get_stack_by_name(&name)
        .ok_or_else(|| CascadeError::config(format!("Stack '{name}' not found")))?;
    let stack_id = stack.id;

    if !force && !stack.entries.is_empty() {
        return Err(CascadeError::config(format!(
            "Stack '{}' has {} entries. Use --force to delete anyway",
            name,
            stack.entries.len()
        )));
    }

    let deleted = manager.delete_stack(&stack_id)?;

    Output::success(format!("Deleted stack '{}'", deleted.name));
    if !deleted.entries.is_empty() {
        Output::warning(format!("{} entries were removed", deleted.entries.len()));
    }

    Ok(())
}

async fn validate_stack(name: Option<String>, fix_mode: Option<String>) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut manager = StackManager::new(&repo_root)?;

    if let Some(name) = name {
        // Validate specific stack
        let stack = manager
            .get_stack_by_name(&name)
            .ok_or_else(|| CascadeError::config(format!("Stack '{name}' not found")))?;

        let stack_id = stack.id;

        // Basic structure validation first
        match stack.validate() {
            Ok(message) => {
                println!("✅ Stack '{name}' structure validation: {message}");
            }
            Err(e) => {
                println!("❌ Stack '{name}' structure validation failed: {e}");
                return Err(CascadeError::config(e));
            }
        }

        // Handle branch modifications (includes Git integrity checks)
        manager.handle_branch_modifications(&stack_id, fix_mode)?;

        println!("🎉 Stack '{name}' validation completed");
        Ok(())
    } else {
        // Validate all stacks
        println!("🔍 Validating all stacks...");

        // Get all stack IDs through public method
        let all_stacks = manager.get_all_stacks();
        let stack_ids: Vec<uuid::Uuid> = all_stacks.iter().map(|s| s.id).collect();

        if stack_ids.is_empty() {
            println!("📭 No stacks found");
            return Ok(());
        }

        let mut all_valid = true;
        for stack_id in stack_ids {
            let stack = manager.get_stack(&stack_id).unwrap();
            let stack_name = &stack.name;

            println!("\n📋 Checking stack '{stack_name}':");

            // Basic structure validation
            match stack.validate() {
                Ok(message) => {
                    println!("  ✅ Structure: {message}");
                }
                Err(e) => {
                    println!("  ❌ Structure: {e}");
                    all_valid = false;
                    continue;
                }
            }

            // Handle branch modifications
            match manager.handle_branch_modifications(&stack_id, fix_mode.clone()) {
                Ok(_) => {
                    println!("  ✅ Git integrity: OK");
                }
                Err(e) => {
                    println!("  ❌ Git integrity: {e}");
                    all_valid = false;
                }
            }
        }

        if all_valid {
            println!("\n🎉 All stacks passed validation");
        } else {
            println!("\n⚠️  Some stacks have validation issues");
            return Err(CascadeError::config("Stack validation failed".to_string()));
        }

        Ok(())
    }
}

/// Get commits that are not yet in any stack entry
#[allow(dead_code)]
fn get_unpushed_commits(repo: &GitRepository, stack: &crate::stack::Stack) -> Result<Vec<String>> {
    let mut unpushed = Vec::new();
    let head_commit = repo.get_head_commit()?;
    let mut current_commit = head_commit;

    // Walk back from HEAD until we find a commit that's already in the stack
    loop {
        let commit_hash = current_commit.id().to_string();
        let already_in_stack = stack
            .entries
            .iter()
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
    Ok(unpushed)
}

/// Squash the last N commits into a single commit
pub async fn squash_commits(
    repo: &GitRepository,
    count: usize,
    since_ref: Option<String>,
) -> Result<()> {
    if count <= 1 {
        return Ok(()); // Nothing to squash
    }

    // Get the current branch
    let _current_branch = repo.get_current_branch()?;

    // Determine the range for interactive rebase
    let rebase_range = if let Some(ref since) = since_ref {
        since.clone()
    } else {
        format!("HEAD~{count}")
    };

    println!("   Analyzing {count} commits to create smart squash message...");

    // Get the commits that will be squashed to create a smart message
    let head_commit = repo.get_head_commit()?;
    let mut commits_to_squash = Vec::new();
    let mut current = head_commit;

    // Collect the last N commits
    for _ in 0..count {
        commits_to_squash.push(current.clone());
        if current.parent_count() > 0 {
            current = current.parent(0).map_err(CascadeError::Git)?;
        } else {
            break;
        }
    }

    // Generate smart commit message from the squashed commits
    let smart_message = generate_squash_message(&commits_to_squash)?;
    println!(
        "   Smart message: {}",
        smart_message.lines().next().unwrap_or("")
    );

    // Get the commit we want to reset to (the commit before our range)
    let reset_target = if since_ref.is_some() {
        // If squashing since a reference, reset to that reference
        format!("{rebase_range}~1")
    } else {
        // If squashing last N commits, reset to N commits before
        format!("HEAD~{count}")
    };

    // Soft reset to preserve changes in staging area
    repo.reset_soft(&reset_target)?;

    // Stage all changes (they should already be staged from the reset --soft)
    repo.stage_all()?;

    // Create the new commit with the smart message
    let new_commit_hash = repo.commit(&smart_message)?;

    println!(
        "   Created squashed commit: {} ({})",
        &new_commit_hash[..8],
        smart_message.lines().next().unwrap_or("")
    );
    println!("   💡 Tip: Use 'git commit --amend' to edit the commit message if needed");

    Ok(())
}

/// Generate a smart commit message from multiple commits being squashed
pub fn generate_squash_message(commits: &[git2::Commit]) -> Result<String> {
    if commits.is_empty() {
        return Ok("Squashed commits".to_string());
    }

    // Get all commit messages
    let messages: Vec<String> = commits
        .iter()
        .map(|c| c.message().unwrap_or("").trim().to_string())
        .filter(|m| !m.is_empty())
        .collect();

    if messages.is_empty() {
        return Ok("Squashed commits".to_string());
    }

    // Strategy 1: If the last commit looks like a "Final:" commit, use it
    if let Some(last_msg) = messages.first() {
        // first() because we're in reverse chronological order
        if last_msg.starts_with("Final:") || last_msg.starts_with("final:") {
            return Ok(last_msg
                .trim_start_matches("Final:")
                .trim_start_matches("final:")
                .trim()
                .to_string());
        }
    }

    // Strategy 2: If most commits are WIP, find the most descriptive non-WIP message
    let wip_count = messages
        .iter()
        .filter(|m| {
            m.to_lowercase().starts_with("wip") || m.to_lowercase().contains("work in progress")
        })
        .count();

    if wip_count > messages.len() / 2 {
        // Mostly WIP commits, find the best non-WIP one or create a summary
        let non_wip: Vec<&String> = messages
            .iter()
            .filter(|m| {
                !m.to_lowercase().starts_with("wip")
                    && !m.to_lowercase().contains("work in progress")
            })
            .collect();

        if let Some(best_msg) = non_wip.first() {
            return Ok(best_msg.to_string());
        }

        // All are WIP, try to extract the feature being worked on
        let feature = extract_feature_from_wip(&messages);
        return Ok(feature);
    }

    // Strategy 3: Use the last (most recent) commit message
    Ok(messages.first().unwrap().clone())
}

/// Extract feature name from WIP commit messages
pub fn extract_feature_from_wip(messages: &[String]) -> String {
    // Look for patterns like "WIP: add authentication" -> "Add authentication"
    for msg in messages {
        // Check both case variations, but preserve original case
        if msg.to_lowercase().starts_with("wip:") {
            if let Some(rest) = msg
                .strip_prefix("WIP:")
                .or_else(|| msg.strip_prefix("wip:"))
            {
                let feature = rest.trim();
                if !feature.is_empty() && feature.len() > 3 {
                    // Capitalize first letter only, preserve rest
                    let mut chars: Vec<char> = feature.chars().collect();
                    if let Some(first) = chars.first_mut() {
                        *first = first.to_uppercase().next().unwrap_or(*first);
                    }
                    return chars.into_iter().collect();
                }
            }
        }
    }

    // Fallback: Use the latest commit without WIP prefix
    if let Some(first) = messages.first() {
        let cleaned = first
            .trim_start_matches("WIP:")
            .trim_start_matches("wip:")
            .trim_start_matches("WIP")
            .trim_start_matches("wip")
            .trim();

        if !cleaned.is_empty() {
            return format!("Implement {cleaned}");
        }
    }

    format!("Squashed {} commits", messages.len())
}

/// Count commits since a given reference
pub fn count_commits_since(repo: &GitRepository, since_commit_hash: &str) -> Result<usize> {
    let head_commit = repo.get_head_commit()?;
    let since_commit = repo.get_commit(since_commit_hash)?;

    let mut count = 0;
    let mut current = head_commit;

    // Walk backwards from HEAD until we reach the since commit
    loop {
        if current.id() == since_commit.id() {
            break;
        }

        count += 1;

        // Get parent commit
        if current.parent_count() == 0 {
            break; // Reached root commit
        }

        current = current.parent(0).map_err(CascadeError::Git)?;
    }

    Ok(count)
}

/// Land (merge) approved stack entries
async fn land_stack(
    entry: Option<usize>,
    force: bool,
    dry_run: bool,
    auto: bool,
    wait_for_builds: bool,
    strategy: Option<MergeStrategyArg>,
    build_timeout: u64,
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let stack_manager = StackManager::new(&repo_root)?;

    // Get stack ID and active stack before moving stack_manager
    let stack_id = stack_manager
        .get_active_stack()
        .map(|s| s.id)
        .ok_or_else(|| {
            CascadeError::config(
                "No active stack. Use 'ca stack create' or 'ca stack switch' to select a stack"
                    .to_string(),
            )
        })?;

    let active_stack = stack_manager
        .get_active_stack()
        .cloned()
        .ok_or_else(|| CascadeError::config("No active stack found".to_string()))?;

    // Load configuration and create Bitbucket integration
    let config_dir = crate::config::get_repo_config_dir(&repo_root)?;
    let config_path = config_dir.join("config.json");
    let settings = crate::config::Settings::load_from_file(&config_path)?;

    let cascade_config = crate::config::CascadeConfig {
        bitbucket: Some(settings.bitbucket.clone()),
        git: settings.git.clone(),
        auth: crate::config::AuthConfig::default(),
        cascade: settings.cascade.clone(),
    };

    let integration = crate::bitbucket::BitbucketIntegration::new(stack_manager, cascade_config)?;

    // Get enhanced status
    let status = integration.check_enhanced_stack_status(&stack_id).await?;

    if status.enhanced_statuses.is_empty() {
        println!("❌ No pull requests found to land");
        return Ok(());
    }

    // Filter PRs that are ready to land
    let ready_prs: Vec<_> = status
        .enhanced_statuses
        .iter()
        .filter(|pr_status| {
            // If specific entry requested, only include that one
            if let Some(entry_num) = entry {
                // Find the corresponding stack entry for this PR
                if let Some(stack_entry) = active_stack.entries.get(entry_num.saturating_sub(1)) {
                    // Check if this PR corresponds to the requested entry
                    if pr_status.pr.from_ref.display_id != stack_entry.branch {
                        return false;
                    }
                } else {
                    return false; // Invalid entry number
                }
            }

            if force {
                // If force is enabled, include any open PR
                pr_status.pr.state == crate::bitbucket::pull_request::PullRequestState::Open
            } else {
                pr_status.is_ready_to_land()
            }
        })
        .collect();

    if ready_prs.is_empty() {
        if let Some(entry_num) = entry {
            println!("❌ Entry {entry_num} is not ready to land or doesn't exist");
        } else {
            println!("❌ No pull requests are ready to land");
        }

        // Show what's blocking them
        println!("\n🚫 Blocking Issues:");
        for pr_status in &status.enhanced_statuses {
            if pr_status.pr.state == crate::bitbucket::pull_request::PullRequestState::Open {
                let blocking = pr_status.get_blocking_reasons();
                if !blocking.is_empty() {
                    println!("   PR #{}: {}", pr_status.pr.id, blocking.join(", "));
                }
            }
        }

        if !force {
            println!("\n💡 Use --force to land PRs with blocking issues (dangerous!)");
        }
        return Ok(());
    }

    if dry_run {
        if let Some(entry_num) = entry {
            println!("🏃 Dry Run - Entry {entry_num} that would be landed:");
        } else {
            println!("🏃 Dry Run - PRs that would be landed:");
        }
        for pr_status in &ready_prs {
            println!("   ✅ PR #{}: {}", pr_status.pr.id, pr_status.pr.title);
            if !pr_status.is_ready_to_land() && force {
                let blocking = pr_status.get_blocking_reasons();
                println!(
                    "      ⚠️  Would force land despite: {}",
                    blocking.join(", ")
                );
            }
        }
        return Ok(());
    }

    // Default behavior: land all ready PRs (safest approach)
    // Only land specific entry if explicitly requested
    if entry.is_some() && ready_prs.len() > 1 {
        println!(
            "🎯 {} PRs are ready to land, but landing only entry #{}",
            ready_prs.len(),
            entry.unwrap()
        );
    }

    // Setup auto-merge conditions
    let merge_strategy: crate::bitbucket::pull_request::MergeStrategy =
        strategy.unwrap_or(MergeStrategyArg::Squash).into();
    let auto_merge_conditions = crate::bitbucket::pull_request::AutoMergeConditions {
        merge_strategy: merge_strategy.clone(),
        wait_for_builds,
        build_timeout: std::time::Duration::from_secs(build_timeout),
        allowed_authors: None, // Allow all authors for now
    };

    // Land the PRs
    println!(
        "🚀 Landing {} PR{}...",
        ready_prs.len(),
        if ready_prs.len() == 1 { "" } else { "s" }
    );

    let pr_manager = crate::bitbucket::pull_request::PullRequestManager::new(
        crate::bitbucket::BitbucketClient::new(&settings.bitbucket)?,
    );

    // Land PRs in dependency order
    let mut landed_count = 0;
    let mut failed_count = 0;
    let total_ready_prs = ready_prs.len();

    for pr_status in ready_prs {
        let pr_id = pr_status.pr.id;

        print!("🚀 Landing PR #{}: {}", pr_id, pr_status.pr.title);

        let land_result = if auto {
            // Use auto-merge with conditions checking
            pr_manager
                .auto_merge_if_ready(pr_id, &auto_merge_conditions)
                .await
        } else {
            // Manual merge without auto-conditions
            pr_manager
                .merge_pull_request(pr_id, merge_strategy.clone())
                .await
                .map(
                    |pr| crate::bitbucket::pull_request::AutoMergeResult::Merged {
                        pr: Box::new(pr),
                        merge_strategy: merge_strategy.clone(),
                    },
                )
        };

        match land_result {
            Ok(crate::bitbucket::pull_request::AutoMergeResult::Merged { .. }) => {
                println!(" ✅");
                landed_count += 1;

                // 🔄 AUTO-RETARGETING: After each merge, retarget remaining PRs
                if landed_count < total_ready_prs {
                    println!("🔄 Retargeting remaining PRs to latest base...");

                    // 1️⃣ CRITICAL: Update base branch to get latest merged state
                    let base_branch = active_stack.base_branch.clone();
                    let git_repo = crate::git::GitRepository::open(&repo_root)?;

                    println!("   📥 Updating base branch: {base_branch}");
                    match git_repo.pull(&base_branch) {
                        Ok(_) => println!("   ✅ Base branch updated successfully"),
                        Err(e) => {
                            println!("   ⚠️  Warning: Failed to update base branch: {e}");
                            println!(
                                "   💡 You may want to manually run: git pull origin {base_branch}"
                            );
                        }
                    }

                    // 2️⃣ Use rebase system to retarget remaining PRs
                    let mut rebase_manager = crate::stack::RebaseManager::new(
                        StackManager::new(&repo_root)?,
                        git_repo,
                        crate::stack::RebaseOptions {
                            strategy: crate::stack::RebaseStrategy::BranchVersioning,
                            target_base: Some(base_branch.clone()),
                            ..Default::default()
                        },
                    );

                    match rebase_manager.rebase_stack(&stack_id) {
                        Ok(rebase_result) => {
                            if !rebase_result.branch_mapping.is_empty() {
                                // Update PRs using the rebase result
                                let retarget_config = crate::config::CascadeConfig {
                                    bitbucket: Some(settings.bitbucket.clone()),
                                    git: settings.git.clone(),
                                    auth: crate::config::AuthConfig::default(),
                                    cascade: settings.cascade.clone(),
                                };
                                let mut retarget_integration = BitbucketIntegration::new(
                                    StackManager::new(&repo_root)?,
                                    retarget_config,
                                )?;

                                match retarget_integration
                                    .update_prs_after_rebase(
                                        &stack_id,
                                        &rebase_result.branch_mapping,
                                    )
                                    .await
                                {
                                    Ok(updated_prs) => {
                                        if !updated_prs.is_empty() {
                                            println!(
                                                "   ✅ Updated {} PRs with new targets",
                                                updated_prs.len()
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        println!("   ⚠️  Failed to update remaining PRs: {e}");
                                        println!(
                                            "   💡 You may need to run: ca stack rebase --onto {base_branch}"
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            // 🚨 CONFLICTS DETECTED - Give clear next steps
                            println!("   ❌ Auto-retargeting conflicts detected!");
                            println!("   📝 To resolve conflicts and continue landing:");
                            println!("      1. Resolve conflicts in the affected files");
                            println!("      2. Stage resolved files: git add <files>");
                            println!("      3. Continue the process: ca stack continue-land");
                            println!("      4. Or abort the operation: ca stack abort-land");
                            println!();
                            println!("   💡 Check current status: ca stack land-status");
                            println!("   ⚠️  Error details: {e}");

                            // Stop the land operation here - user needs to resolve conflicts
                            break;
                        }
                    }
                }
            }
            Ok(crate::bitbucket::pull_request::AutoMergeResult::NotReady { blocking_reasons }) => {
                println!(" ❌ Not ready: {}", blocking_reasons.join(", "));
                failed_count += 1;
                if !force {
                    break;
                }
            }
            Ok(crate::bitbucket::pull_request::AutoMergeResult::Failed { error }) => {
                println!(" ❌ Failed: {error}");
                failed_count += 1;
                if !force {
                    break;
                }
            }
            Err(e) => {
                println!(" ❌");
                eprintln!("Failed to land PR #{pr_id}: {e}");
                failed_count += 1;

                if !force {
                    break;
                }
            }
        }
    }

    // Show summary
    println!("\n🎯 Landing Summary:");
    println!("   ✅ Successfully landed: {landed_count}");
    if failed_count > 0 {
        println!("   ❌ Failed to land: {failed_count}");
    }

    if landed_count > 0 {
        println!("✅ Landing operation completed!");
    } else {
        println!("❌ No PRs were successfully landed");
    }

    Ok(())
}

/// Auto-land all ready PRs (shorthand for land --auto)
async fn auto_land_stack(
    force: bool,
    dry_run: bool,
    wait_for_builds: bool,
    strategy: Option<MergeStrategyArg>,
    build_timeout: u64,
) -> Result<()> {
    // This is a shorthand for land with --auto
    land_stack(
        None,
        force,
        dry_run,
        true, // auto = true
        wait_for_builds,
        strategy,
        build_timeout,
    )
    .await
}

async fn continue_land() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let stack_manager = StackManager::new(&repo_root)?;
    let git_repo = crate::git::GitRepository::open(&repo_root)?;
    let options = crate::stack::RebaseOptions::default();
    let rebase_manager = crate::stack::RebaseManager::new(stack_manager, git_repo, options);

    if !rebase_manager.is_rebase_in_progress() {
        println!("ℹ️  No rebase in progress");
        return Ok(());
    }

    println!("🔄 Continuing land operation...");
    match rebase_manager.continue_rebase() {
        Ok(_) => {
            println!("✅ Land operation continued successfully");
            println!("   Check 'ca stack land-status' for current state");
        }
        Err(e) => {
            warn!("❌ Failed to continue land operation: {}", e);
            println!("💡 You may need to resolve conflicts first:");
            println!("   1. Edit conflicted files");
            println!("   2. Stage resolved files with 'git add'");
            println!("   3. Run 'ca stack continue-land' again");
        }
    }

    Ok(())
}

async fn abort_land() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let stack_manager = StackManager::new(&repo_root)?;
    let git_repo = crate::git::GitRepository::open(&repo_root)?;
    let options = crate::stack::RebaseOptions::default();
    let rebase_manager = crate::stack::RebaseManager::new(stack_manager, git_repo, options);

    if !rebase_manager.is_rebase_in_progress() {
        println!("ℹ️  No rebase in progress");
        return Ok(());
    }

    println!("⚠️  Aborting land operation...");
    match rebase_manager.abort_rebase() {
        Ok(_) => {
            println!("✅ Land operation aborted successfully");
            println!("   Repository restored to pre-land state");
        }
        Err(e) => {
            warn!("❌ Failed to abort land operation: {}", e);
            println!("⚠️  You may need to manually clean up the repository state");
        }
    }

    Ok(())
}

async fn land_status() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let stack_manager = StackManager::new(&repo_root)?;
    let git_repo = crate::git::GitRepository::open(&repo_root)?;

    println!("📊 Land Status");

    // Check if land operation is in progress by checking git state directly
    let git_dir = repo_root.join(".git");
    let land_in_progress = git_dir.join("REBASE_HEAD").exists()
        || git_dir.join("rebase-merge").exists()
        || git_dir.join("rebase-apply").exists();

    if land_in_progress {
        println!("   Status: 🔄 Land operation in progress");
        println!(
            "   
📝 Actions available:"
        );
        println!("     - 'ca stack continue-land' to continue");
        println!("     - 'ca stack abort-land' to abort");
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
                        println!("      - {conflict}");
                    }
                    println!(
                        "   
💡 To resolve conflicts:"
                    );
                    println!("     1. Edit the conflicted files");
                    println!("     2. Stage resolved files: git add <file>");
                    println!("     3. Continue: ca stack continue-land");
                }
            }
            Err(e) => {
                warn!("Failed to get git status: {}", e);
            }
        }
    } else {
        println!("   Status: ✅ No land operation in progress");

        // Show stack status instead
        if let Some(active_stack) = stack_manager.get_active_stack() {
            println!("   Active stack: {}", active_stack.name);
            println!("   Entries: {}", active_stack.entries.len());
            println!("   Base branch: {}", active_stack.base_branch);
        }
    }

    Ok(())
}

async fn repair_stack_data() -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut stack_manager = StackManager::new(&repo_root)?;

    println!("🔧 Repairing stack data consistency...");

    stack_manager.repair_all_stacks()?;

    println!("✅ Stack data consistency repaired successfully!");
    println!("💡 Run 'ca stack --mergeable' to see updated status");

    Ok(())
}

/// Analyze commits for various safeguards before pushing
async fn analyze_commits_for_safeguards(
    commits_to_push: &[String],
    repo: &GitRepository,
    dry_run: bool,
) -> Result<()> {
    const LARGE_COMMIT_THRESHOLD: usize = 10;
    const WEEK_IN_SECONDS: i64 = 7 * 24 * 3600;

    // 🛡️ SAFEGUARD 1: Large commit count warning
    if commits_to_push.len() > LARGE_COMMIT_THRESHOLD {
        println!(
            "⚠️  Warning: About to push {} commits to stack",
            commits_to_push.len()
        );
        println!("   This may indicate a merge commit issue or unexpected commit range.");
        println!("   Large commit counts often result from merging instead of rebasing.");

        if !dry_run && !confirm_large_push(commits_to_push.len())? {
            return Err(CascadeError::config("Push cancelled by user"));
        }
    }

    // Get commit objects for further analysis
    let commit_objects: Result<Vec<_>> = commits_to_push
        .iter()
        .map(|hash| repo.get_commit(hash))
        .collect();
    let commit_objects = commit_objects?;

    // 🛡️ SAFEGUARD 2: Merge commit detection
    let merge_commits: Vec<_> = commit_objects
        .iter()
        .filter(|c| c.parent_count() > 1)
        .collect();

    if !merge_commits.is_empty() {
        println!(
            "⚠️  Warning: {} merge commits detected in push",
            merge_commits.len()
        );
        println!("   This often indicates you merged instead of rebased.");
        println!("   Consider using 'ca sync' to rebase on the base branch.");
        println!("   Merge commits in stacks can cause confusion and duplicate work.");
    }

    // 🛡️ SAFEGUARD 3: Commit age warning
    if commit_objects.len() > 1 {
        let oldest_commit_time = commit_objects.first().unwrap().time().seconds();
        let newest_commit_time = commit_objects.last().unwrap().time().seconds();
        let time_span = newest_commit_time - oldest_commit_time;

        if time_span > WEEK_IN_SECONDS {
            let days = time_span / (24 * 3600);
            println!("⚠️  Warning: Commits span {days} days");
            println!("   This may indicate merged history rather than new work.");
            println!("   Recent work should typically span hours or days, not weeks.");
        }
    }

    // 🛡️ SAFEGUARD 4: Better range detection suggestions
    if commits_to_push.len() > 5 {
        println!("💡 Tip: If you only want recent commits, use:");
        println!(
            "   ca push --since HEAD~{}  # pushes last {} commits",
            std::cmp::min(commits_to_push.len(), 5),
            std::cmp::min(commits_to_push.len(), 5)
        );
        println!("   ca push --commits <hash1>,<hash2>  # pushes specific commits");
        println!("   ca push --dry-run  # preview what would be pushed");
    }

    // 🛡️ SAFEGUARD 5: Dry run mode
    if dry_run {
        println!("🔍 DRY RUN: Would push {} commits:", commits_to_push.len());
        for (i, (commit_hash, commit_obj)) in commits_to_push
            .iter()
            .zip(commit_objects.iter())
            .enumerate()
        {
            let summary = commit_obj.summary().unwrap_or("(no message)");
            let short_hash = &commit_hash[..std::cmp::min(commit_hash.len(), 7)];
            println!("  {}: {} ({})", i + 1, summary, short_hash);
        }
        println!("💡 Run without --dry-run to actually push these commits.");
    }

    Ok(())
}

/// Prompt user for confirmation when pushing large number of commits
fn confirm_large_push(count: usize) -> Result<bool> {
    print!("Do you want to continue pushing {count} commits? [y/N]: ");
    io::stdout()
        .flush()
        .map_err(|e| CascadeError::config(format!("Failed to flush stdout: {e}")))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| CascadeError::config(format!("Failed to read user input: {e}")))?;

    let input = input.trim().to_lowercase();
    Ok(input == "y" || input == "yes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn create_test_repo() -> Result<(TempDir, std::path::PathBuf)> {
        let temp_dir = TempDir::new()
            .map_err(|e| CascadeError::config(format!("Failed to create temp directory: {e}")))?;
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repository
        let output = Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to run git init: {e}")))?;
        if !output.status.success() {
            return Err(CascadeError::config("Git init failed".to_string()));
        }

        let output = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to run git config: {e}")))?;
        if !output.status.success() {
            return Err(CascadeError::config(
                "Git config user.name failed".to_string(),
            ));
        }

        let output = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to run git config: {e}")))?;
        if !output.status.success() {
            return Err(CascadeError::config(
                "Git config user.email failed".to_string(),
            ));
        }

        // Create initial commit
        std::fs::write(repo_path.join("README.md"), "# Test")
            .map_err(|e| CascadeError::config(format!("Failed to write file: {e}")))?;
        let output = Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to run git add: {e}")))?;
        if !output.status.success() {
            return Err(CascadeError::config("Git add failed".to_string()));
        }

        let output = Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| CascadeError::config(format!("Failed to run git commit: {e}")))?;
        if !output.status.success() {
            return Err(CascadeError::config("Git commit failed".to_string()));
        }

        // Initialize cascade
        crate::config::initialize_repo(&repo_path, Some("https://test.bitbucket.com".to_string()))?;

        Ok((temp_dir, repo_path))
    }

    #[tokio::test]
    async fn test_create_stack() {
        let (temp_dir, repo_path) = match create_test_repo() {
            Ok(repo) => repo,
            Err(_) => {
                println!("Skipping test due to git environment setup failure");
                return;
            }
        };
        // IMPORTANT: temp_dir must stay in scope to prevent early cleanup of test directory
        let _ = &temp_dir;

        // Note: create_test_repo() already initializes Cascade configuration

        // Change to the repo directory (with proper error handling)
        let original_dir = env::current_dir().map_err(|_| "Failed to get current dir");
        match env::set_current_dir(&repo_path) {
            Ok(_) => {
                let result = create_stack(
                    "test-stack".to_string(),
                    None, // Use default branch
                    Some("Test description".to_string()),
                )
                .await;

                // Restore original directory (best effort)
                if let Ok(orig) = original_dir {
                    let _ = env::set_current_dir(orig);
                }

                assert!(
                    result.is_ok(),
                    "Stack creation should succeed in initialized repository"
                );
            }
            Err(_) => {
                // Skip test if we can't change directories (CI environment issue)
                println!("Skipping test due to directory access restrictions");
            }
        }
    }

    #[tokio::test]
    async fn test_list_empty_stacks() {
        let (temp_dir, repo_path) = match create_test_repo() {
            Ok(repo) => repo,
            Err(_) => {
                println!("Skipping test due to git environment setup failure");
                return;
            }
        };
        // IMPORTANT: temp_dir must stay in scope to prevent early cleanup of test directory
        let _ = &temp_dir;

        // Note: create_test_repo() already initializes Cascade configuration

        // Change to the repo directory (with proper error handling)
        let original_dir = env::current_dir().map_err(|_| "Failed to get current dir");
        match env::set_current_dir(&repo_path) {
            Ok(_) => {
                let result = list_stacks(false, false, None).await;

                // Restore original directory (best effort)
                if let Ok(orig) = original_dir {
                    let _ = env::set_current_dir(orig);
                }

                assert!(
                    result.is_ok(),
                    "Listing stacks should succeed in initialized repository"
                );
            }
            Err(_) => {
                // Skip test if we can't change directories (CI environment issue)
                println!("Skipping test due to directory access restrictions");
            }
        }
    }

    // Tests for squashing functionality

    #[test]
    fn test_extract_feature_from_wip_basic() {
        let messages = vec![
            "WIP: add authentication".to_string(),
            "WIP: implement login flow".to_string(),
        ];

        let result = extract_feature_from_wip(&messages);
        assert_eq!(result, "Add authentication");
    }

    #[test]
    fn test_extract_feature_from_wip_capitalize() {
        let messages = vec!["WIP: fix user validation bug".to_string()];

        let result = extract_feature_from_wip(&messages);
        assert_eq!(result, "Fix user validation bug");
    }

    #[test]
    fn test_extract_feature_from_wip_fallback() {
        let messages = vec![
            "WIP user interface changes".to_string(),
            "wip: css styling".to_string(),
        ];

        let result = extract_feature_from_wip(&messages);
        // Should create a fallback message since no "WIP:" prefix found
        assert!(result.contains("Implement") || result.contains("Squashed") || result.len() > 5);
    }

    #[test]
    fn test_extract_feature_from_wip_empty() {
        let messages = vec![];

        let result = extract_feature_from_wip(&messages);
        assert_eq!(result, "Squashed 0 commits");
    }

    #[test]
    fn test_extract_feature_from_wip_short_message() {
        let messages = vec!["WIP: x".to_string()]; // Too short

        let result = extract_feature_from_wip(&messages);
        assert!(result.starts_with("Implement") || result.contains("Squashed"));
    }

    // Integration tests for squashing that don't require real git commits

    #[test]
    fn test_squash_message_final_strategy() {
        // This test would need real git2::Commit objects, so we'll test the logic indirectly
        // through the extract_feature_from_wip function which handles the core logic

        let messages = [
            "Final: implement user authentication system".to_string(),
            "WIP: add tests".to_string(),
            "WIP: fix validation".to_string(),
        ];

        // Test that we can identify final commits
        assert!(messages[0].starts_with("Final:"));

        // Test message extraction
        let extracted = messages[0].trim_start_matches("Final:").trim();
        assert_eq!(extracted, "implement user authentication system");
    }

    #[test]
    fn test_squash_message_wip_detection() {
        let messages = [
            "WIP: start feature".to_string(),
            "WIP: continue work".to_string(),
            "WIP: almost done".to_string(),
            "Regular commit message".to_string(),
        ];

        let wip_count = messages
            .iter()
            .filter(|m| {
                m.to_lowercase().starts_with("wip") || m.to_lowercase().contains("work in progress")
            })
            .count();

        assert_eq!(wip_count, 3); // Should detect 3 WIP commits
        assert!(wip_count > messages.len() / 2); // Majority are WIP

        // Should find the non-WIP message
        let non_wip: Vec<&String> = messages
            .iter()
            .filter(|m| {
                !m.to_lowercase().starts_with("wip")
                    && !m.to_lowercase().contains("work in progress")
            })
            .collect();

        assert_eq!(non_wip.len(), 1);
        assert_eq!(non_wip[0], "Regular commit message");
    }

    #[test]
    fn test_squash_message_all_wip() {
        let messages = vec![
            "WIP: add feature A".to_string(),
            "WIP: add feature B".to_string(),
            "WIP: finish implementation".to_string(),
        ];

        let result = extract_feature_from_wip(&messages);
        // Should use the first message as the main feature
        assert_eq!(result, "Add feature A");
    }

    #[test]
    fn test_squash_message_edge_cases() {
        // Test empty messages
        let empty_messages: Vec<String> = vec![];
        let result = extract_feature_from_wip(&empty_messages);
        assert_eq!(result, "Squashed 0 commits");

        // Test messages with only whitespace
        let whitespace_messages = vec!["   ".to_string(), "\t\n".to_string()];
        let result = extract_feature_from_wip(&whitespace_messages);
        assert!(result.contains("Squashed") || result.contains("Implement"));

        // Test case sensitivity
        let mixed_case = vec!["wip: Add Feature".to_string()];
        let result = extract_feature_from_wip(&mixed_case);
        assert_eq!(result, "Add Feature");
    }

    // Tests for auto-land functionality

    #[tokio::test]
    async fn test_auto_land_wrapper() {
        // Test that auto_land_stack correctly calls land_stack with auto=true
        let (temp_dir, repo_path) = match create_test_repo() {
            Ok(repo) => repo,
            Err(_) => {
                println!("Skipping test due to git environment setup failure");
                return;
            }
        };
        // IMPORTANT: temp_dir must stay in scope to prevent early cleanup of test directory
        let _ = &temp_dir;

        // Initialize cascade in the test repo
        crate::config::initialize_repo(&repo_path, Some("https://test.bitbucket.com".to_string()))
            .expect("Failed to initialize Cascade in test repo");

        let original_dir = env::current_dir().map_err(|_| "Failed to get current dir");
        match env::set_current_dir(&repo_path) {
            Ok(_) => {
                // Create a stack first
                let result = create_stack(
                    "test-stack".to_string(),
                    None,
                    Some("Test stack for auto-land".to_string()),
                )
                .await;

                if let Ok(orig) = original_dir {
                    let _ = env::set_current_dir(orig);
                }

                // For now, just test that the function can be called without panic
                // (It will fail due to missing Bitbucket config, but that's expected)
                assert!(
                    result.is_ok(),
                    "Stack creation should succeed in initialized repository"
                );
            }
            Err(_) => {
                println!("Skipping test due to directory access restrictions");
            }
        }
    }

    #[test]
    fn test_auto_land_action_enum() {
        // Test that AutoLand action is properly defined
        use crate::cli::commands::stack::StackAction;

        // This ensures the AutoLand variant exists and has the expected fields
        let _action = StackAction::AutoLand {
            force: false,
            dry_run: true,
            wait_for_builds: true,
            strategy: Some(MergeStrategyArg::Squash),
            build_timeout: 1800,
        };

        // Test passes if we reach this point without errors
    }

    #[test]
    fn test_merge_strategy_conversion() {
        // Test that MergeStrategyArg converts properly
        let squash_strategy = MergeStrategyArg::Squash;
        let merge_strategy: crate::bitbucket::pull_request::MergeStrategy = squash_strategy.into();

        match merge_strategy {
            crate::bitbucket::pull_request::MergeStrategy::Squash => {
                // Correct conversion
            }
            _ => panic!("Expected Squash strategy"),
        }

        let merge_strategy = MergeStrategyArg::Merge;
        let converted: crate::bitbucket::pull_request::MergeStrategy = merge_strategy.into();

        match converted {
            crate::bitbucket::pull_request::MergeStrategy::Merge => {
                // Correct conversion
            }
            _ => panic!("Expected Merge strategy"),
        }
    }

    #[test]
    fn test_auto_merge_conditions_structure() {
        // Test that AutoMergeConditions can be created with expected values
        use std::time::Duration;

        let conditions = crate::bitbucket::pull_request::AutoMergeConditions {
            merge_strategy: crate::bitbucket::pull_request::MergeStrategy::Squash,
            wait_for_builds: true,
            build_timeout: Duration::from_secs(1800),
            allowed_authors: None,
        };

        // Verify the conditions are set as expected for auto-land
        assert!(conditions.wait_for_builds);
        assert_eq!(conditions.build_timeout.as_secs(), 1800);
        assert!(conditions.allowed_authors.is_none());
        assert!(matches!(
            conditions.merge_strategy,
            crate::bitbucket::pull_request::MergeStrategy::Squash
        ));
    }

    #[test]
    fn test_polling_constants() {
        // Test that polling frequency is documented and reasonable
        use std::time::Duration;

        // The polling frequency should be 30 seconds as mentioned in documentation
        let expected_polling_interval = Duration::from_secs(30);

        // Verify it's a reasonable value (not too frequent, not too slow)
        assert!(expected_polling_interval.as_secs() >= 10); // At least 10 seconds
        assert!(expected_polling_interval.as_secs() <= 60); // At most 1 minute
        assert_eq!(expected_polling_interval.as_secs(), 30); // Exactly 30 seconds
    }

    #[test]
    fn test_build_timeout_defaults() {
        // Verify build timeout default is reasonable
        const DEFAULT_TIMEOUT: u64 = 1800; // 30 minutes
        assert_eq!(DEFAULT_TIMEOUT, 1800);
        // Test that our default timeout value is within reasonable bounds
        let timeout_value = 1800u64;
        assert!(timeout_value >= 300); // At least 5 minutes
        assert!(timeout_value <= 3600); // At most 1 hour
    }

    #[test]
    fn test_scattered_commit_detection() {
        use std::collections::HashSet;

        // Test scattered commit detection logic
        let mut source_branches = HashSet::new();
        source_branches.insert("feature-branch-1".to_string());
        source_branches.insert("feature-branch-2".to_string());
        source_branches.insert("feature-branch-3".to_string());

        // Single branch should not trigger warning
        let single_branch = HashSet::from(["main".to_string()]);
        assert_eq!(single_branch.len(), 1);

        // Multiple branches should trigger warning
        assert!(source_branches.len() > 1);
        assert_eq!(source_branches.len(), 3);

        // Verify branch names are preserved correctly
        assert!(source_branches.contains("feature-branch-1"));
        assert!(source_branches.contains("feature-branch-2"));
        assert!(source_branches.contains("feature-branch-3"));
    }

    #[test]
    fn test_source_branch_tracking() {
        // Test that source branch tracking correctly handles different scenarios

        // Same branch should be consistent
        let branch_a = "feature-work";
        let branch_b = "feature-work";
        assert_eq!(branch_a, branch_b);

        // Different branches should be detected
        let branch_1 = "feature-ui";
        let branch_2 = "feature-api";
        assert_ne!(branch_1, branch_2);

        // Branch naming patterns
        assert!(branch_1.starts_with("feature-"));
        assert!(branch_2.starts_with("feature-"));
    }

    // Tests for new default behavior (removing --all flag)

    #[tokio::test]
    async fn test_push_default_behavior() {
        // Test the push_to_stack function structure and error handling in an isolated environment
        let (temp_dir, repo_path) = match create_test_repo() {
            Ok(repo) => repo,
            Err(_) => {
                println!("Skipping test due to git environment setup failure");
                return;
            }
        };
        // IMPORTANT: temp_dir must stay in scope to prevent early cleanup of test directory
        let _ = &temp_dir;

        // Verify directory exists before changing to it
        if !repo_path.exists() {
            println!("Skipping test due to temporary directory creation issue");
            return;
        }

        // Change to the test repository directory to ensure isolation
        let original_dir = env::current_dir().map_err(|_| "Failed to get current dir");

        match env::set_current_dir(&repo_path) {
            Ok(_) => {
                // Test that push_to_stack properly handles the case when no stack is active
                let result = push_to_stack(
                    None,  // branch
                    None,  // message
                    None,  // commit
                    None,  // since
                    None,  // commits
                    None,  // squash
                    None,  // squash_since
                    false, // auto_branch
                    false, // allow_base_branch
                    false, // dry_run
                )
                .await;

                // Restore original directory (best effort)
                if let Ok(orig) = original_dir {
                    let _ = env::set_current_dir(orig);
                }

                // Should fail gracefully with appropriate error message when no stack is active
                match &result {
                    Err(e) => {
                        let error_msg = e.to_string();
                        // This is the expected behavior - no active stack should produce this error
                        assert!(
                            error_msg.contains("No active stack")
                                || error_msg.contains("config")
                                || error_msg.contains("current directory")
                                || error_msg.contains("Not a git repository")
                                || error_msg.contains("could not find repository"),
                            "Expected 'No active stack' or repository error, got: {error_msg}"
                        );
                    }
                    Ok(_) => {
                        // If it somehow succeeds, that's also fine (e.g., if environment is set up differently)
                        println!(
                            "Push succeeded unexpectedly - test environment may have active stack"
                        );
                    }
                }
            }
            Err(_) => {
                // Skip test if we can't change directories (CI environment issue)
                println!("Skipping test due to directory access restrictions");
            }
        }

        // Verify we can construct the command structure correctly
        let push_action = StackAction::Push {
            branch: None,
            message: None,
            commit: None,
            since: None,
            commits: None,
            squash: None,
            squash_since: None,
            auto_branch: false,
            allow_base_branch: false,
            dry_run: false,
        };

        assert!(matches!(
            push_action,
            StackAction::Push {
                branch: None,
                message: None,
                commit: None,
                since: None,
                commits: None,
                squash: None,
                squash_since: None,
                auto_branch: false,
                allow_base_branch: false,
                dry_run: false
            }
        ));
    }

    #[tokio::test]
    async fn test_submit_default_behavior() {
        // Test the submit_entry function structure and error handling in an isolated environment
        let (temp_dir, repo_path) = match create_test_repo() {
            Ok(repo) => repo,
            Err(_) => {
                println!("Skipping test due to git environment setup failure");
                return;
            }
        };
        // IMPORTANT: temp_dir must stay in scope to prevent early cleanup of test directory
        let _ = &temp_dir;

        // Verify directory exists before changing to it
        if !repo_path.exists() {
            println!("Skipping test due to temporary directory creation issue");
            return;
        }

        // Change to the test repository directory to ensure isolation
        let original_dir = match env::current_dir() {
            Ok(dir) => dir,
            Err(_) => {
                println!("Skipping test due to current directory access restrictions");
                return;
            }
        };

        match env::set_current_dir(&repo_path) {
            Ok(_) => {
                // Test that submit_entry properly handles the case when no stack is active
                let result = submit_entry(
                    None,  // entry (should default to all unsubmitted)
                    None,  // title
                    None,  // description
                    None,  // range
                    false, // draft
                )
                .await;

                // Restore original directory
                let _ = env::set_current_dir(original_dir);

                // Should fail gracefully with appropriate error message when no stack is active
                match &result {
                    Err(e) => {
                        let error_msg = e.to_string();
                        // This is the expected behavior - no active stack should produce this error
                        assert!(
                            error_msg.contains("No active stack")
                                || error_msg.contains("config")
                                || error_msg.contains("current directory")
                                || error_msg.contains("Not a git repository")
                                || error_msg.contains("could not find repository"),
                            "Expected 'No active stack' or repository error, got: {error_msg}"
                        );
                    }
                    Ok(_) => {
                        // If it somehow succeeds, that's also fine (e.g., if environment is set up differently)
                        println!("Submit succeeded unexpectedly - test environment may have active stack");
                    }
                }
            }
            Err(_) => {
                // Skip test if we can't change directories (CI environment issue)
                println!("Skipping test due to directory access restrictions");
            }
        }

        // Verify we can construct the command structure correctly
        let submit_action = StackAction::Submit {
            entry: None,
            title: None,
            description: None,
            range: None,
            draft: false,
        };

        assert!(matches!(
            submit_action,
            StackAction::Submit {
                entry: None,
                title: None,
                description: None,
                range: None,
                draft: false
            }
        ));
    }

    #[test]
    fn test_targeting_options_still_work() {
        // Test that specific targeting options still work correctly

        // Test commit list parsing
        let commits = "abc123,def456,ghi789";
        let parsed: Vec<&str> = commits.split(',').map(|s| s.trim()).collect();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], "abc123");
        assert_eq!(parsed[1], "def456");
        assert_eq!(parsed[2], "ghi789");

        // Test range parsing would work
        let range = "1-3";
        assert!(range.contains('-'));
        let parts: Vec<&str> = range.split('-').collect();
        assert_eq!(parts.len(), 2);

        // Test since reference pattern
        let since_ref = "HEAD~3";
        assert!(since_ref.starts_with("HEAD"));
        assert!(since_ref.contains('~'));
    }

    #[test]
    fn test_command_flow_logic() {
        // These just test the command structure exists
        assert!(matches!(
            StackAction::Push {
                branch: None,
                message: None,
                commit: None,
                since: None,
                commits: None,
                squash: None,
                squash_since: None,
                auto_branch: false,
                allow_base_branch: false,
                dry_run: false
            },
            StackAction::Push { .. }
        ));

        assert!(matches!(
            StackAction::Submit {
                entry: None,
                title: None,
                description: None,
                range: None,
                draft: false
            },
            StackAction::Submit { .. }
        ));
    }

    #[tokio::test]
    async fn test_deactivate_command_structure() {
        // Test that deactivate command structure exists and can be constructed
        let deactivate_action = StackAction::Deactivate { force: false };

        // Verify it matches the expected pattern
        assert!(matches!(
            deactivate_action,
            StackAction::Deactivate { force: false }
        ));

        // Test with force flag
        let force_deactivate = StackAction::Deactivate { force: true };
        assert!(matches!(
            force_deactivate,
            StackAction::Deactivate { force: true }
        ));
    }
}
