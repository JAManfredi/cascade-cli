pub mod commands;

use clap::{Parser, Subcommand};
use clap_complete::Shell;
use crate::errors::Result;
use commands::stack::StackAction;

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
    Stack {
        #[command(subcommand)]
        action: StackAction,
    },
    
    /// Show repository status
    Status,
    
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
}

/// Git hooks actions
#[derive(Debug, Subcommand)]
pub enum HooksAction {
    /// Install all Cascade Git hooks
    Install,
    
    /// Uninstall all Cascade Git hooks
    Uninstall,
    
    /// Show Git hooks status
    Status,
    
    /// Install a specific hook
    Add {
        /// Hook name (post-commit, pre-push, commit-msg, prepare-commit-msg)
        hook: String,
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
            Commands::Init { bitbucket_url, force } => {
                commands::init::run(bitbucket_url, force).await
            }
            Commands::Config { action } => {
                commands::config::run(action).await
            }
            Commands::Stack { action } => {
                commands::stack::run(action).await
            }
            Commands::Status => {
                commands::status::run().await
            }
            Commands::Version => {
                commands::version::run().await
            }
            Commands::Doctor => {
                commands::doctor::run().await
            }
            
            Commands::Completions { action } => {
                match action {
                    CompletionsAction::Generate { shell } => {
                        commands::completions::generate_completions(shell)
                    }
                    CompletionsAction::Install { shell } => {
                        commands::completions::install_completions(shell)
                    }
                    CompletionsAction::Status => {
                        commands::completions::show_completions_status()
                    }
                }
            }
            
            Commands::Setup { force } => {
                commands::setup::run(force).await
            }
            
            Commands::Tui => {
                commands::tui::run().await
            }
            
            Commands::Hooks { action } => {
                match action {
                    HooksAction::Install => {
                        commands::hooks::install().await
                    }
                    HooksAction::Uninstall => {
                        commands::hooks::uninstall().await
                    }
                    HooksAction::Status => {
                        commands::hooks::status().await
                    }
                    HooksAction::Add { hook } => {
                        commands::hooks::install_hook(&hook).await
                    }
                    HooksAction::Remove { hook } => {
                        commands::hooks::uninstall_hook(&hook).await
                    }
                }
            }

            Commands::Viz { action } => {
                match action {
                    VizAction::Stack { name, format, output, compact, no_colors } => {
                        commands::viz::show_stack(name.clone(), format.clone(), output.clone(), compact, no_colors).await
                    }
                    VizAction::Deps { format, output, compact, no_colors } => {
                        commands::viz::show_dependencies(format.clone(), output.clone(), compact, no_colors).await
                    }
                }
            }
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