pub mod commands;

use clap::{Parser, Subcommand};
use crate::errors::Result;
use commands::stack::StackAction;

#[derive(Parser)]
#[command(name = "cc")]
#[command(about = "Cascade CLI - Stacked diffs for Bitbucket Server")]
#[command(version)]
pub struct App {
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

impl App {
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