use crate::cli::output::Output;
use crate::errors::Result;

/// Show version information
pub async fn run() -> Result<()> {
    Output::section("Cascade CLI");
    Output::sub_item(format!("Version: {}", env!("CARGO_PKG_VERSION")));
    Output::sub_item(format!("Authors: {}", env!("CARGO_PKG_AUTHORS")));
    Output::sub_item(format!("Homepage: {}", env!("CARGO_PKG_HOMEPAGE")));
    Output::sub_item(format!("Description: {}", env!("CARGO_PKG_DESCRIPTION")));

    Output::section("Build Information");
    Output::sub_item(format!("Rust version: {}", env!("CARGO_PKG_RUST_VERSION")));
    Output::sub_item(format!("Target: {}", std::env::consts::ARCH));
    Output::sub_item(format!("OS: {}", std::env::consts::OS));

    #[cfg(debug_assertions)]
    Output::sub_item("Build type: Debug");
    #[cfg(not(debug_assertions))]
    Output::sub_item("Build type: Release");

    Output::section("Key Dependencies");
    Output::sub_item("clap: 4.0+");
    Output::sub_item("git2: 0.18+");
    Output::sub_item("reqwest: 0.11+");
    Output::sub_item("tokio: 1.0+");
    Output::sub_item("serde: 1.0+");

    Output::section("Links");
    Output::sub_item("Repository: https://github.com/your-org/cascade-cli");
    Output::sub_item("Issues: https://github.com/your-org/cascade-cli/issues");
    Output::sub_item("Documentation: https://github.com/your-org/cascade-cli/wiki");

    Output::section("Quick Start");
    Output::sub_item("Initialize repository: ca init");
    Output::sub_item("Show help: ca --help");
    Output::sub_item("Check status: ca status");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_version_command() {
        let result = run().await;
        assert!(result.is_ok());
    }
}
