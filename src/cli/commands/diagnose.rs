use crate::cli::output::Output;
use crate::errors::Result;
use crate::git::GitRepository;
use std::path::Path;

/// Run diagnostic checks for git2 TLS/SSH support
pub async fn run() -> Result<()> {
    Output::section("Cascade CLI - Git2 Diagnostics");

    // Check git2 features
    let version = git2::Version::get();
    Output::section("Git2 Feature Support");

    if version.https() {
        Output::success("HTTPS/TLS support: YES");
    } else {
        Output::error("HTTPS/TLS support: NO");
    }

    if version.ssh() {
        Output::success("SSH support: YES");
    } else {
        Output::error("SSH support: NO");
    }

    // Get libgit2 version
    let libgit2_version = version.libgit2_version();
    Output::sub_item(format!(
        "libgit2 version: {}.{}.{}",
        libgit2_version.0, libgit2_version.1, libgit2_version.2
    ));

    println!();

    // Check current repository if we're in one
    if let Ok(repo) = GitRepository::open(Path::new(".")) {
        Output::section("Current Repository Analysis");

        // Use the built-in diagnostic method
        repo.diagnose_git2_support()?;

        // Check remote URLs
        if let Ok(remote_url) = repo.get_remote_url("origin") {
            Output::section("Remote Configuration");
            Output::sub_item(format!("Origin URL: {remote_url}"));

            if remote_url.starts_with("https://") {
                if version.https() {
                    Output::success("HTTPS remote with TLS support - should work!");
                } else {
                    Output::error("HTTPS remote but NO TLS support - will fallback to git CLI");
                }
            } else if remote_url.starts_with("git@") || remote_url.starts_with("ssh://") {
                if version.ssh() {
                    Output::success("SSH remote with SSH support - should work!");
                } else {
                    Output::error("SSH remote but NO SSH support - will fallback to git CLI");
                }
            }
        }
    } else {
        Output::info("Not in a git repository - skipping repository-specific checks");
    }

    // Provide recommendations
    Output::section("Recommendations");

    if !version.https() || !version.ssh() {
        Output::error("MISSING FEATURES DETECTED:");
        Output::sub_item("Your git2 is missing TLS/SSH support.");
        Output::sub_item("This causes performance issues due to git CLI fallbacks.");
        println!();
        Output::tip("TO FIX: Update Cargo.toml git2 dependency:");
        Output::command_example("git2 = { version = \"0.20.2\", features = [\"vendored-libgit2\", \"https\", \"ssh\"] }");
        println!();
        Output::success("BENEFITS: Direct git2 operations (faster, more reliable)");
    } else {
        Output::success("git2 has full TLS/SSH support!");
        Output::sub_item("If you're still experiencing issues, they may be:");
        Output::bullet("Network connectivity problems");
        Output::bullet("Authentication/credential issues");
        Output::bullet("Corporate firewall/proxy settings");
        Output::bullet("SSL certificate verification problems");
    }

    Ok(())
}
