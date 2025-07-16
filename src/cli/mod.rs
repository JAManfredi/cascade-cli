pub mod commands;
pub mod output;

use crate::errors::Result;
use clap::{Parser, Subcommand};
use clap_complete::Shell;
use commands::entry::EntryAction;
use commands::stack::StackAction;
use commands::{MergeStrategyArg, RebaseStrategyArg};

#[derive(Parser)]
#[command(name = "ca")]
#[command(about = "Cascade CLI - Stacked Diffs for Bitbucket")]
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

/// Commands available in the CLI
#[derive(Debug, Subcommand)]
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

    /// Diagnose git2 TLS/SSH support issues
    Diagnose,

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
        /// Show what would be pushed without actually pushing
        #[arg(long)]
        dry_run: bool,
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
        #[arg(value_hint = clap::ValueHint::Other)]
        name: String,
    },

    /// Deactivate the current stack - turn off stack mode (shortcut for 'stacks deactivate')
    Deactivate {
        /// Force deactivation without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Submit a stack entry for review (shortcut for 'stacks submit')
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

    /// Validate stack integrity and handle branch modifications (shortcut for 'stacks validate')
    Validate {
        /// Name of the stack (defaults to active stack)
        name: Option<String>,
        /// Auto-fix mode: incorporate, split, or reset
        #[arg(long)]
        fix: Option<String>,
    },

    /// Internal command for shell completion (hidden)
    #[command(hide = true)]
    CompletionHelper {
        #[command(subcommand)]
        action: CompletionHelperAction,
    },
}

/// Git hooks actions
#[derive(Debug, Subcommand)]
pub enum HooksAction {
    /// Install Cascade Git hooks
    Install {
        /// Install all hooks including post-commit (default: essential hooks only)
        #[arg(long)]
        all: bool,

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

/// Hidden completion helper actions
#[derive(Debug, Subcommand)]
pub enum CompletionHelperAction {
    /// List available stack names
    StackNames,
}

#[derive(Debug, Subcommand)]
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

        // Initialize git2 to use system certificates by default
        // This ensures we work out-of-the-box in corporate environments
        // just like git CLI and other modern dev tools (Graphite, Sapling, Phabricator)
        self.init_git2_ssl()?;

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
            Commands::Diagnose => commands::diagnose::run().await,

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
                    all,
                    skip_checks,
                    allow_main_branch,
                    yes,
                    force,
                } => {
                    if all {
                        // Install all hooks including post-commit
                        commands::hooks::install_with_options(
                            skip_checks,
                            allow_main_branch,
                            yes,
                            force,
                        )
                        .await
                    } else {
                        // Install essential hooks by default (excludes post-commit)
                        // Users can install post-commit separately with 'ca hooks add post-commit'
                        commands::hooks::install_essential().await
                    }
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

            Commands::Stack { verbose, mergeable } => {
                commands::stack::show(verbose, mergeable).await
            }

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
                dry_run,
            } => {
                commands::stack::push(
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

            Commands::Pop { keep_branch } => commands::stack::pop(keep_branch).await,

            Commands::Land {
                entry,
                force,
                dry_run,
                auto,
                wait_for_builds,
                strategy,
                build_timeout,
            } => {
                commands::stack::land(
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

            Commands::Autoland {
                force,
                dry_run,
                wait_for_builds,
                strategy,
                build_timeout,
            } => {
                commands::stack::autoland(force, dry_run, wait_for_builds, strategy, build_timeout)
                    .await
            }

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

            Commands::Submit {
                entry,
                title,
                description,
                range,
                draft,
            } => {
                // Delegate to the stacks submit functionality
                let submit_action = StackAction::Submit {
                    entry,
                    title,
                    description,
                    range,
                    draft,
                };
                commands::stack::run(submit_action).await
            }

            Commands::Validate { name, fix } => {
                // Delegate to the stacks validate functionality
                let validate_action = StackAction::Validate { name, fix };
                commands::stack::run(validate_action).await
            }

            Commands::CompletionHelper { action } => handle_completion_helper(action).await,
        }
    }

    /// Initialize git2 to use system certificates by default
    /// This makes Cascade work like git CLI in corporate environments
    fn init_git2_ssl(&self) -> Result<()> {
        // Only import SSL functions on platforms that use them
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        use git2::opts::{set_ssl_cert_dir, set_ssl_cert_file};

        // Configure git2 to use system certificate store
        // This matches behavior of git CLI and tools like Graphite/Sapling
        tracing::debug!("Initializing git2 SSL configuration with system certificates");

        // Try to use system certificate locations
        // On macOS: /etc/ssl/cert.pem, /usr/local/etc/ssl/cert.pem
        // On Linux: /etc/ssl/certs/ca-certificates.crt, /etc/ssl/certs/ca-bundle.crt
        // On Windows: Uses Windows certificate store automatically

        #[cfg(target_os = "macos")]
        {
            // macOS certificate locations (certificate files)
            let cert_files = [
                "/etc/ssl/cert.pem",
                "/usr/local/etc/ssl/cert.pem",
                "/opt/homebrew/etc/ca-certificates/cert.pem",
            ];

            for cert_path in &cert_files {
                if std::path::Path::new(cert_path).exists() {
                    tracing::debug!("Using macOS system certificates from: {}", cert_path);
                    if let Err(e) = unsafe { set_ssl_cert_file(cert_path) } {
                        tracing::trace!(
                            "SSL cert file {} not supported by TLS backend: {}",
                            cert_path,
                            e
                        );
                    } else {
                        return Ok(());
                    }
                }
            }

            // Fallback to certificate directories
            let cert_dirs = ["/etc/ssl/certs", "/usr/local/etc/ssl/certs"];

            for cert_dir in &cert_dirs {
                if std::path::Path::new(cert_dir).exists() {
                    tracing::debug!("Using macOS system certificate directory: {}", cert_dir);
                    if let Err(e) = unsafe { set_ssl_cert_dir(cert_dir) } {
                        tracing::trace!(
                            "SSL cert directory {} not supported by TLS backend: {}",
                            cert_dir,
                            e
                        );
                    } else {
                        return Ok(());
                    }
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Linux certificate files
            let cert_files = [
                "/etc/ssl/certs/ca-certificates.crt", // Debian/Ubuntu
                "/etc/ssl/certs/ca-bundle.crt",       // RHEL/CentOS
                "/etc/pki/tls/certs/ca-bundle.crt",   // Fedora/RHEL
                "/etc/ssl/ca-bundle.pem",             // OpenSUSE
            ];

            for cert_path in &cert_files {
                if std::path::Path::new(cert_path).exists() {
                    tracing::debug!("Using Linux system certificates from: {}", cert_path);
                    if let Err(e) = unsafe { set_ssl_cert_file(cert_path) } {
                        tracing::trace!(
                            "SSL cert file {} not supported by TLS backend: {}",
                            cert_path,
                            e
                        );
                    } else {
                        return Ok(());
                    }
                }
            }

            // Fallback to certificate directories
            let cert_dirs = ["/etc/ssl/certs", "/etc/pki/tls/certs"];

            for cert_dir in &cert_dirs {
                if std::path::Path::new(cert_dir).exists() {
                    tracing::debug!("Using Linux system certificate directory: {}", cert_dir);
                    if let Err(e) = unsafe { set_ssl_cert_dir(cert_dir) } {
                        tracing::trace!(
                            "SSL cert directory {} not supported by TLS backend: {}",
                            cert_dir,
                            e
                        );
                    } else {
                        return Ok(());
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Windows uses system certificate store automatically via git2's default configuration
            tracing::debug!("Using Windows system certificate store (automatic)");
        }

        tracing::debug!("System SSL certificate configuration complete");
        tracing::debug!(
            "Note: SSL warnings from libgit2 are normal - git CLI fallback will be used if needed"
        );
        Ok(())
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

/// Handle completion helper commands
async fn handle_completion_helper(action: CompletionHelperAction) -> Result<()> {
    match action {
        CompletionHelperAction::StackNames => {
            use crate::git::find_repository_root;
            use crate::stack::StackManager;
            use std::env;

            // Try to get stack names, but silently fail if not in a repository
            if let Ok(current_dir) = env::current_dir() {
                if let Ok(repo_root) = find_repository_root(&current_dir) {
                    if let Ok(manager) = StackManager::new(&repo_root) {
                        for (_, name, _, _, _) in manager.list_stacks() {
                            println!("{name}");
                        }
                    }
                }
            }
            Ok(())
        }
    }
}
