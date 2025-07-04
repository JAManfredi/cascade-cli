pub mod commands;

use crate::errors::Result;
use clap::{Parser, Subcommand};
use clap_complete::Shell;
use commands::entry::EntryAction;
use commands::stack::StackAction;
use commands::{MergeStrategyArg, RebaseStrategyArg};

use crate::cli::commands::{config, hooks, init, stack, status, diagnose};

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
    /// Initialize cascade in current repository
    Init {
        /// Bitbucket server URL (e.g., https://bitbucket.company.com)
        bitbucket_url: Option<String>,
        /// Force initialization even if already initialized
        #[arg(long)]
        force: bool,
    },
    /// Stack management commands
    #[command(subcommand)]
    Stack(stack::StackCommands),
    /// Install git hooks
    #[command(subcommand)]
    Hooks(hooks::HookCommands),
    /// Display repository status
    Status,
    /// Configuration management
    #[command(subcommand)]
    Config(config::ConfigCommands),
    /// Diagnose git2 TLS/SSH support issues
    Diagnose,
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
        match self.command {
            Commands::Init { bitbucket_url, force } => init::run(bitbucket_url, force).await,
            Commands::Stack(action) => stack::run(action).await,
            Commands::Hooks(action) => hooks::run(action).await,
            Commands::Status => status::run().await,
            Commands::Config(action) => config::run(action).await,
            Commands::Diagnose => diagnose::run().await,
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
        tracing::info!(
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
