use clap::Parser;
use cascade_cli::cli::App;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = App::parse();
    app.run().await.map_err(|e| anyhow::Error::new(e))
}