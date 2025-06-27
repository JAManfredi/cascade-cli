pub mod commands;

use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "cc")]
#[command(about = "Cascade CLI - Stacked diffs for Bitbucket Server")]
#[command(version)]
pub struct App {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize repository for Cascade
    Init {
        #[arg(long)]
        bitbucket_url: Option<String>,
    },
    /// Show version information
    Version,
}

impl App {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Commands::Init { bitbucket_url } => {
                println!("Initializing Cascade repository...");
                if let Some(url) = bitbucket_url {
                    println!("Bitbucket URL: {}", url);
                }
                Ok(())
            }
            Commands::Version => {
                println!("Cascade CLI v{}", env!("CARGO_PKG_VERSION"));
                Ok(())
            }
        }
    }
}