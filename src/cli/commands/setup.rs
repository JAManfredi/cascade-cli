use crate::config::{get_repo_config_dir, initialize_repo, Settings};
use crate::errors::{CascadeError, Result};
use crate::git::GitRepository;
use dialoguer::{theme::ColorfulTheme, Confirm, Input};
use std::env;
use tracing::{info, warn};

/// Run the interactive setup wizard
pub async fn run(force: bool) -> Result<()> {
    println!("🌊 Welcome to Cascade CLI Setup!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("This wizard will help you configure Cascade for your repository.\n");

    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

    // Step 1: Check Git repository
    println!("🔍 Step 1: Checking Git repository...");
    let git_repo = match GitRepository::open(&current_dir) {
        Ok(repo) => {
            println!("   ✅ Git repository found");
            repo
        }
        Err(_) => {
            return Err(CascadeError::config(
                "No Git repository found. Please run this command from within a Git repository.",
            ));
        }
    };

    // Step 2: Check if already initialized
    let config_dir = get_repo_config_dir(&current_dir)?;
    if config_dir.exists() && !force {
        let reinitialize = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Cascade is already initialized. Do you want to reconfigure?")
            .default(false)
            .interact()
            .map_err(|e| CascadeError::config(format!("Input error: {}", e)))?;

        if !reinitialize {
            println!("✅ Setup cancelled. Run with --force to reconfigure.");
            return Ok(());
        }
    }

    // Step 3: Detect Bitbucket from remotes
    println!("\n🔍 Step 2: Detecting Bitbucket configuration...");
    let auto_config = detect_bitbucket_config(&git_repo)?;

    if let Some((url, project, repo)) = &auto_config {
        println!("   ✅ Detected Bitbucket configuration:");
        println!("      Server: {}", url);
        println!("      Project: {}", project);
        println!("      Repository: {}", repo);
    } else {
        println!("   ⚠️  Could not auto-detect Bitbucket configuration");
    }

    // Step 4: Interactive configuration
    println!("\n⚙️  Step 3: Configure Bitbucket settings...");
    let bitbucket_config = configure_bitbucket_interactive(auto_config).await?;

    // Step 5: Initialize repository
    println!("\n🚀 Step 4: Initializing Cascade...");
    initialize_repo(&current_dir, Some(bitbucket_config.url.clone()))?;

    // Step 6: Save configuration
    let config_path = config_dir.join("config.json");
    let mut settings = Settings::load_from_file(&config_path).unwrap_or_default();

    settings.bitbucket.url = bitbucket_config.url;
    settings.bitbucket.project = bitbucket_config.project;
    settings.bitbucket.repo = bitbucket_config.repo;
    settings.bitbucket.token = bitbucket_config.token;

    settings.save_to_file(&config_path)?;

    // Step 7: Test connection (optional)
    println!("\n🔌 Step 5: Testing connection...");
    if let Some(ref token) = settings.bitbucket.token {
        if !token.is_empty() {
            match test_bitbucket_connection(&settings).await {
                Ok(_) => {
                    println!("   ✅ Connection successful!");
                }
                Err(e) => {
                    warn!("   ⚠️  Connection test failed: {}", e);
                    println!("   💡 You can test the connection later with: cc doctor");
                }
            }
        } else {
            println!("   ⚠️  No token provided - skipping connection test");
        }
    } else {
        println!("   ⚠️  No token provided - skipping connection test");
    }

    // Step 8: Setup completions (optional)
    println!("\n🚀 Step 6: Shell completions...");
    let install_completions = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Would you like to install shell completions?")
        .default(true)
        .interact()
        .map_err(|e| CascadeError::config(format!("Input error: {}", e)))?;

    if install_completions {
        match crate::cli::commands::completions::install_completions(None) {
            Ok(_) => {
                println!("   ✅ Shell completions installed");
            }
            Err(e) => {
                warn!("   ⚠️  Failed to install completions: {}", e);
                println!("   💡 You can install them later with: cc completions install");
            }
        }
    }

    // Success summary
    println!("\n🎉 Setup Complete!");
    println!("━━━━━━━━━━━━━━━━━");
    println!("Cascade CLI is now configured for your repository.");
    println!();
    println!("💡 Next steps:");
    println!("   1. Create your first stack: cc stack create \"My Feature\"");
    println!("   2. Push commits to the stack: cc stack push");
    println!("   3. Submit for review: cc stack submit");
    println!("   4. Check status: cc stack status");
    println!();
    println!("📚 Learn more:");
    println!("   • Run 'cc --help' for all commands");
    println!("   • Run 'cc doctor' to verify your setup");
    println!("   • Visit the documentation for advanced usage");

    Ok(())
}

#[derive(Debug)]
struct BitbucketConfig {
    url: String,
    project: String,
    repo: String,
    token: Option<String>,
}

/// Detect Bitbucket configuration from Git remotes
fn detect_bitbucket_config(git_repo: &GitRepository) -> Result<Option<(String, String, String)>> {
    // Get the remote URL
    let remote_url = match git_repo.get_remote_url("origin") {
        Ok(url) => url,
        Err(_) => return Ok(None),
    };

    // Parse different URL formats
    if let Some(config) = parse_bitbucket_url(&remote_url) {
        Ok(Some(config))
    } else {
        Ok(None)
    }
}

/// Parse Bitbucket URL from various formats
fn parse_bitbucket_url(url: &str) -> Option<(String, String, String)> {
    // Handle SSH format: git@bitbucket.example.com:PROJECT/repo.git
    if url.starts_with("git@") {
        if let Some(parts) = url.split('@').nth(1) {
            if let Some((host, path)) = parts.split_once(':') {
                let base_url = format!("https://{}", host);
                if let Some((project, repo)) = path.split_once('/') {
                    let repo_name = repo.strip_suffix(".git").unwrap_or(repo);
                    return Some((base_url, project.to_string(), repo_name.to_string()));
                }
            }
        }
    }

    // Handle HTTPS format: https://bitbucket.example.com/scm/PROJECT/repo.git
    if url.starts_with("https://") {
        if let Ok(parsed_url) = url::Url::parse(url) {
            if let Some(host) = parsed_url.host_str() {
                let base_url = format!("{}://{}", parsed_url.scheme(), host);
                let path = parsed_url.path();

                // Bitbucket Server format: /scm/PROJECT/repo.git
                if path.starts_with("/scm/") {
                    let path_parts: Vec<&str> =
                        path.trim_start_matches("/scm/").split('/').collect();
                    if path_parts.len() >= 2 {
                        let project = path_parts[0];
                        let repo = path_parts[1].strip_suffix(".git").unwrap_or(path_parts[1]);
                        return Some((base_url, project.to_string(), repo.to_string()));
                    }
                }

                // Generic format: /PROJECT/repo.git
                let path_parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
                if path_parts.len() >= 2 {
                    let project = path_parts[0];
                    let repo = path_parts[1].strip_suffix(".git").unwrap_or(path_parts[1]);
                    return Some((base_url, project.to_string(), repo.to_string()));
                }
            }
        }
    }

    None
}

/// Interactive Bitbucket configuration
async fn configure_bitbucket_interactive(
    auto_config: Option<(String, String, String)>,
) -> Result<BitbucketConfig> {
    let theme = ColorfulTheme::default();

    // Server URL
    let default_url = auto_config
        .as_ref()
        .map(|(url, _, _)| url.as_str())
        .unwrap_or("");
    let url: String = Input::with_theme(&theme)
        .with_prompt("Bitbucket Server URL")
        .with_initial_text(default_url)
        .validate_with(|input: &String| -> std::result::Result<(), &str> {
            if input.starts_with("http://") || input.starts_with("https://") {
                Ok(())
            } else {
                Err("URL must start with http:// or https://")
            }
        })
        .interact_text()
        .map_err(|e| CascadeError::config(format!("Input error: {}", e)))?;

    // Project key
    let default_project = auto_config
        .as_ref()
        .map(|(_, project, _)| project.as_str())
        .unwrap_or("");
    let project: String = Input::with_theme(&theme)
        .with_prompt("Project key (usually uppercase)")
        .with_initial_text(default_project)
        .validate_with(|input: &String| -> std::result::Result<(), &str> {
            if input.trim().is_empty() {
                Err("Project key cannot be empty")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .map_err(|e| CascadeError::config(format!("Input error: {}", e)))?;

    // Repository slug
    let default_repo = auto_config
        .as_ref()
        .map(|(_, _, repo)| repo.as_str())
        .unwrap_or("");
    let repo: String = Input::with_theme(&theme)
        .with_prompt("Repository slug")
        .with_initial_text(default_repo)
        .validate_with(|input: &String| -> std::result::Result<(), &str> {
            if input.trim().is_empty() {
                Err("Repository slug cannot be empty")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .map_err(|e| CascadeError::config(format!("Input error: {}", e)))?;

    // Authentication token
    println!("\n🔐 Authentication Setup");
    println!("   Cascade needs a Personal Access Token to interact with Bitbucket.");
    println!(
        "   You can create one at: {}/plugins/servlet/access-tokens/manage",
        url
    );
    println!("   Required permissions: Repository Read, Repository Write");

    let configure_token = Confirm::with_theme(&theme)
        .with_prompt("Configure authentication token now?")
        .default(true)
        .interact()
        .map_err(|e| CascadeError::config(format!("Input error: {}", e)))?;

    let token = if configure_token {
        let token: String = Input::with_theme(&theme)
            .with_prompt("Personal Access Token")
            .interact_text()
            .map_err(|e| CascadeError::config(format!("Input error: {}", e)))?;

        if token.trim().is_empty() {
            None
        } else {
            Some(token.trim().to_string())
        }
    } else {
        println!("   💡 You can configure the token later with:");
        println!("      cc config set bitbucket.token YOUR_TOKEN");
        None
    };

    Ok(BitbucketConfig {
        url,
        project,
        repo,
        token,
    })
}

/// Test Bitbucket connection
async fn test_bitbucket_connection(settings: &Settings) -> Result<()> {
    use crate::bitbucket::BitbucketClient;

    let client = BitbucketClient::new(&settings.bitbucket)?;

    // Try to fetch repository info
    match client.get_repository_info().await {
        Ok(_) => {
            info!("Successfully connected to Bitbucket");
            Ok(())
        }
        Err(e) => Err(CascadeError::config(format!(
            "Failed to connect to Bitbucket: {}",
            e
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bitbucket_ssh_url() {
        let url = "git@bitbucket.example.com:MYPROJECT/my-repo.git";
        let result = parse_bitbucket_url(url);
        assert_eq!(
            result,
            Some((
                "https://bitbucket.example.com".to_string(),
                "MYPROJECT".to_string(),
                "my-repo".to_string()
            ))
        );
    }

    #[test]
    fn test_parse_bitbucket_https_url() {
        let url = "https://bitbucket.example.com/scm/MYPROJECT/my-repo.git";
        let result = parse_bitbucket_url(url);
        assert_eq!(
            result,
            Some((
                "https://bitbucket.example.com".to_string(),
                "MYPROJECT".to_string(),
                "my-repo".to_string()
            ))
        );
    }

    #[test]
    fn test_parse_generic_https_url() {
        let url = "https://git.example.com/MYPROJECT/my-repo.git";
        let result = parse_bitbucket_url(url);
        assert_eq!(
            result,
            Some((
                "https://git.example.com".to_string(),
                "MYPROJECT".to_string(),
                "my-repo".to_string()
            ))
        );
    }
}
