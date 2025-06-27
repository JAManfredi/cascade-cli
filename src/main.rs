use clap::Parser;
use cascade_cli::cli::App;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with a simple format
    tracing_subscriber::fmt::init();
    
    let app = App::parse();
    app.run().await
}