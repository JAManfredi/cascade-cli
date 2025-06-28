use cascade_cli::cli::Cli;
use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    cli.run().await.map_err(|e| anyhow::Error::new(e))
}
