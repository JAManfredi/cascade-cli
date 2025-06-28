pub mod commands;

use crate::errors::Result;
use clap::{Parser, Subcommand};
use clap_complete::Shell;
use commands::entry::EntryAction;
use commands::stack::StackAction;
use commands::{MergeStrategyArg, RebaseStrategyArg};

#[derive(Parser)]
#[command(name = "cc")]
#[command(about = "Cascade CLI - Stacked diffs for Bitbucket Server")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose logging
    #[arg(long, short, global = true)]
    pub verbose: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize repository for Cascade
    Init {
        /// Bitbucket Server URL
        #[arg(long)]
        bitbucket_url: Option<String>,

        /// Force initialization even if already initialized
        #[arg(long)]
        force: bool,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Stack management
    Stacks {
        #[command(subcommand)]
        action: StackAction,
    },

    /// Entry management and editing
    Entry {
        #[command(subcommand)]
        action: EntryAction,
    },

    /// Show repository overview and all stacks
    Repo,

    /// Show version information  
    Version,

    /// Check repository health and configuration
    Doctor,

    /// Generate shell completions
    Completions {
        #[command(subcommand)]
        action: CompletionsAction,
    },

    /// Interactive setup wizard
    Setup {
        /// Force reconfiguration if already initialized
        #[arg(long)]
        force: bool,
    },

    /// Launch interactive TUI for stack management
    Tui,

    /// Git hooks management
    Hooks {
        #[command(subcommand)]
        action: HooksAction,
    },

    /// Visualize stacks and dependencies
    Viz {
        #[command(subcommand)]
        action: VizAction,
    },

    // Stack command shortcuts for commonly used operations
    /// Show current stack details
    Stack {
        /// Show detailed pull request information
        #[arg(short, long)]
        verbose: bool,
        /// Show mergability status for all PRs
        #[arg(short, long)]
        mergeable: bool,
    },

    /// Push current commit to the top of the stack (shortcut for 'stack push')
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
        /// Squash last N commits into one before pushing
        #[arg(long)]
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
    },

    /// Pop the top commit from the stack (shortcut for 'stack pop')
    Pop {
        /// Keep the branch (don't delete it)
        #[arg(long)]
        keep_branch: bool,
    },

    /// Land (merge) approved stack entries (shortcut for 'stack land')
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

    /// Auto-land all ready PRs (shortcut for 'stack autoland')
    Autoland {
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

    /// Sync stack with remote repository (shortcut for 'stack sync')
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

    /// Rebase stack on updated base branch (shortcut for 'stack rebase')
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

    /// Switch to a different stack (shortcut for 'stacks switch')
    Switch {
        /// Name of the stack to switch to
        name: String,
    },

    /// Deactivate the current stack - turn off stack mode (shortcut for 'stacks deactivate')
    Deactivate {
        /// Force deactivation without confirmation
        #[arg(long)]
        force: bool,
    },
}

/// Git hooks actions
#[derive(Debug, Subcommand)]
pub enum HooksAction {
    /// Install all Cascade Git hooks
    Install {
        /// Skip prerequisite checks (repository type, configuration validation)
        #[arg(long)]
        skip_checks: bool,

        /// Allow installation on main/master branches (not recommended)
        #[arg(long)]
        allow_main_branch: bool,

        /// Skip confirmation prompt
        #[arg(long, short)]
        yes: bool,

        /// Force installation even if checks fail (not recommended)
        #[arg(long)]
        force: bool,
    },

    /// Uninstall all Cascade Git hooks
    Uninstall,

    /// Show Git hooks status
    Status,

    /// Install a specific hook
    Add {
        /// Hook name (post-commit, pre-push, commit-msg, prepare-commit-msg)
        hook: String,

        /// Skip prerequisite checks
        #[arg(long)]
        skip_checks: bool,

        /// Force installation even if checks fail
        #[arg(long)]
        force: bool,
    },

    /// Remove a specific hook
    Remove {
        /// Hook name (post-commit, pre-push, commit-msg, prepare-commit-msg)
        hook: String,
    },
}

/// Visualization actions
#[derive(Debug, Subcommand)]
pub enum VizAction {
    /// Show stack diagram
    Stack {
        /// Stack name (defaults to active stack)
        name: Option<String>,
        /// Output format (ascii, mermaid, dot, plantuml)
        #[arg(long, short)]
        format: Option<String>,
        /// Output file path
        #[arg(long, short)]
        output: Option<String>,
        /// Compact mode (less details)
        #[arg(long)]
        compact: bool,
        /// Disable colors
        #[arg(long)]
        no_colors: bool,
    },

    /// Show dependency graph of all stacks
    Deps {
        /// Output format (ascii, mermaid, dot, plantuml)
        #[arg(long, short)]
        format: Option<String>,
        /// Output file path
        #[arg(long, short)]
        output: Option<String>,
        /// Compact mode (less details)
        #[arg(long)]
        compact: bool,
        /// Disable colors
        #[arg(long)]
        no_colors: bool,
    },
}

/// Shell completion actions
#[derive(Debug, Subcommand)]
pub enum CompletionsAction {
    /// Generate completions for a shell
    Generate {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Install completions for available shells
    Install {
        /// Specific shell to install for
        #[arg(long, value_enum)]
        shell: Option<Shell>,
    },

    /// Show completion installation status
    Status,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Set a configuration value
    Set {
        /// Configuration key (e.g., bitbucket.url)
        key: String,
        /// Configuration value
        value: String,
    },

    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// List all configuration values
    List,

    /// Remove a configuration value
    Unset {
        /// Configuration key
        key: String,
    },
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        // Set up logging based on verbosity
        self.setup_logging();

        match self.command {
            Commands::Init {
                bitbucket_url,
                force,
            } => commands::init::run(bitbucket_url, force).await,
            Commands::Config { action } => commands::config::run(action).await,
            Commands::Stacks { action } => commands::stack::run(action).await,
            Commands::Entry { action } => commands::entry::run(action).await,
            Commands::Repo => commands::status::run().await,
            Commands::Version => commands::version::run().await,
            Commands::Doctor => commands::doctor::run().await,

            Commands::Completions { action } => match action {
                CompletionsAction::Generate { shell } => {
                    commands::completions::generate_completions(shell)
                }
                CompletionsAction::Install { shell } => {
                    commands::completions::install_completions(shell)
                }
                CompletionsAction::Status => commands::completions::show_completions_status(),
            },

            Commands::Setup { force } => commands::setup::run(force).await,

            Commands::Tui => commands::tui::run().await,

            Commands::Hooks { action } => match action {
                HooksAction::Install {
                    skip_checks,
                    allow_main_branch,
                    yes,
                    force,
                } => {
                    commands::hooks::install_with_options(
                        skip_checks,
                        allow_main_branch,
                        yes,
                        force,
                    )
                    .await
                }
                HooksAction::Uninstall => commands::hooks::uninstall().await,
                HooksAction::Status => commands::hooks::status().await,
                HooksAction::Add {
                    hook,
                    skip_checks,
                    force,
                } => commands::hooks::install_hook_with_options(&hook, skip_checks, force).await,
                HooksAction::Remove { hook } => commands::hooks::uninstall_hook(&hook).await,
            },

            Commands::Viz { action } => match action {
                VizAction::Stack {
                    name,
                    format,
                    output,
                    compact,
                    no_colors,
                } => {
                    commands::viz::show_stack(
                        name.clone(),
                        format.clone(),
                        output.clone(),
                        compact,
                        no_colors,
                    )
                    .await
                }
                VizAction::Deps {
                    format,
                    output,
                    compact,
                    no_colors,
                } => {
                    commands::viz::show_dependencies(
                        format.clone(),
                        output.clone(),
                        compact,
                        no_colors,
                    )
                    .await
                }
            },

            Commands::Stack { verbose, mergeable } => commands::stack::show(verbose, mergeable).await,

            Commands::Push {
                branch,
                message,
                commit,
                since,
                commits,
                squash,
                squash_since,
                auto_branch,
                allow_base_branch,
            } => commands::stack::push(branch, message, commit, since, commits, squash, squash_since, auto_branch, allow_base_branch).await,

            Commands::Pop { keep_branch } => commands::stack::pop(keep_branch).await,

            Commands::Land {
                entry,
                force,
                dry_run,
                auto,
                wait_for_builds,
                strategy,
                build_timeout,
            } => commands::stack::land(entry, force, dry_run, auto, wait_for_builds, strategy, build_timeout).await,

            Commands::Autoland {
                force,
                dry_run,
                wait_for_builds,
                strategy,
                build_timeout,
            } => commands::stack::autoland(force, dry_run, wait_for_builds, strategy, build_timeout).await,

            Commands::Sync {
                force,
                skip_cleanup,
                interactive,
            } => commands::stack::sync(force, skip_cleanup, interactive).await,

            Commands::Rebase {
                interactive,
                onto,
                strategy,
            } => commands::stack::rebase(interactive, onto, strategy).await,

            Commands::Switch { name } => commands::stack::switch(name).await,

            Commands::Deactivate { force } => commands::stack::deactivate(force).await,
        }
    }

    fn setup_logging(&self) {
        let level = if self.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        };

        let subscriber = tracing_subscriber::fmt()
            .with_max_level(level)
            .with_target(false)
            .without_time();

        if self.no_color {
            subscriber.with_ansi(false).init();
        } else {
            subscriber.init();
        }
    }
}
