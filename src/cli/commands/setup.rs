use crate::cli::output::Output;
use crate::config::{get_repo_config_dir, initialize_repo, Settings};
use crate::errors::{CascadeError, Result};
use crate::git::{find_repository_root, GitRepository};
use dialoguer::{theme::ColorfulTheme, Confirm, Input};
use std::env;
use tracing::{info, warn};

/// Run the interactive setup wizard
pub async fn run(force: bool) -> Result<()> {
    Output::section("Welcome to Cascade CLI Setup!");
    Output::divider();
    Output::info("This wizard will help you configure Cascade for your repository.");
    println!();

    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    // Step 1: Find Git repository root
    Output::progress("Step 1: Finding Git repository...");
    let repo_root = find_repository_root(&current_dir).map_err(|_| {
        CascadeError::config(
            "No Git repository found. Please run this command from within a Git repository.",
        )
    })?;

    Output::success(format!("Git repository found at: {}", repo_root.display()));

    let git_repo = GitRepository::open(&repo_root)?;

    // Step 2: Check if already initialized
    let config_dir = get_repo_config_dir(&repo_root)?;
    if config_dir.exists() && !force {
        let reinitialize = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Cascade is already initialized. Do you want to reconfigure?")
            .default(false)
            .interact()
            .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

        if !reinitialize {
            Output::success("Setup cancelled. Run with --force to reconfigure.");
            return Ok(());
        }
    }

    // Step 3: Configure Git user settings
    Output::progress("Step 2: Configuring Git user settings...");
    configure_git_user(&git_repo).await?;

    // Step 4: Detect Bitbucket from remotes
    Output::progress("Step 3: Detecting Bitbucket configuration...");
    let auto_config = detect_bitbucket_config(&git_repo)?;

    if let Some((url, project, repo)) = &auto_config {
        Output::success("Detected Bitbucket configuration:");
        Output::sub_item(format!("Server: {url}"));
        Output::sub_item(format!("Project: {project}"));
        Output::sub_item(format!("Repository: {repo}"));
    } else {
        Output::warning("Could not auto-detect Bitbucket configuration");
    }

    // Step 5: Interactive configuration
    Output::progress("Step 4: Configure Bitbucket settings");
    let bitbucket_config = configure_bitbucket_interactive(auto_config).await?;

    // Step 6: Initialize repository (using repo root, not current dir)
    Output::progress("Step 5: Initializing Cascade");
    initialize_repo(&repo_root, Some(bitbucket_config.url.clone()))?;

    // Step 7: Save configuration
    let config_path = config_dir.join("config.json");
    let mut settings = Settings::load_from_file(&config_path).unwrap_or_default();

    settings.bitbucket.url = bitbucket_config.url;
    settings.bitbucket.project = bitbucket_config.project;
    settings.bitbucket.repo = bitbucket_config.repo;
    settings.bitbucket.token = bitbucket_config.token;

    settings.save_to_file(&config_path)?;

    // Step 8: Test connection (optional)
    Output::progress("Step 6: Testing connection");
    if let Some(ref token) = settings.bitbucket.token {
        if !token.is_empty() {
            match test_bitbucket_connection(&settings).await {
                Ok(_) => {
                    Output::success("Connection successful!");
                }
                Err(e) => {
                    warn!("   ⚠️  Connection test failed: {}", e);
                    Output::tip("You can test the connection later with: ca doctor");
                }
            }
        } else {
            Output::warning("No token provided - skipping connection test");
        }
    } else {
        Output::warning("No token provided - skipping connection test");
    }

    // Step 9: Setup completions (optional)
    Output::progress("Step 7: Shell completions");
    let install_completions = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Would you like to install shell completions?")
        .default(true)
        .interact()
        .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

    if install_completions {
        match crate::cli::commands::completions::install_completions(None) {
            Ok(_) => {
                Output::success("Shell completions installed");
            }
            Err(e) => {
                warn!("   ⚠️  Failed to install completions: {}", e);
                Output::tip("You can install them later with: ca completions install");
            }
        }
    }

    // Step 10: Install Git hooks (recommended)
    Output::progress("Step 8: Git hooks");
    let install_hooks = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Would you like to install Git hooks for enhanced workflow?")
        .default(true)
        .interact()
        .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

    if install_hooks {
        match crate::cli::commands::hooks::install_essential().await {
            Ok(_) => {
                Output::success("Essential Git hooks installed");
                Output::tip("Hooks installed: pre-push, commit-msg, prepare-commit-msg");
                Output::tip(
                    "Optional: Install post-commit hook with 'ca hooks install post-commit'",
                );
                Output::tip("See docs/HOOKS.md for details");
            }
            Err(e) => {
                warn!("   ⚠️  Failed to install hooks: {}", e);
                if e.to_string().contains("Git hooks directory not found") {
                    Output::tip("This doesn't appear to be a Git repository.");
                    println!("      Please ensure you're running this command from within a Git repository.");
                    println!("      You can initialize git with: git init");
                } else {
                    Output::tip("You can install them later with: ca hooks install");
                }
            }
        }
    }

    // Step 11: Configure PR description template (optional)
    Output::progress("Step 9: PR Description Template");
    let setup_template = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(
            "Would you like to configure a PR description template? (will be used for ALL PRs)",
        )
        .default(false)
        .interact()
        .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

    if setup_template {
        configure_pr_template(&config_path).await?;
    } else {
        Output::tip("You can configure a PR template later with:");
        Output::command_example("ca config set cascade.pr_description_template \"Your template\"");
    }

    // Success summary
    Output::section("Setup Complete!");
    Output::success("Cascade CLI is now fully configured for your repository.");
    println!();
    Output::info("Configuration includes:");
    Output::bullet("✅ Git user settings (name and email)");
    Output::bullet("✅ Bitbucket Server integration");
    Output::bullet("✅ Essential Git hooks for enhanced workflow");
    Output::bullet("✅ Shell completions (if selected)");
    println!();
    Output::tip("Next steps:");
    Output::bullet("Create your first stack: ca stack create \"My Feature\"");
    Output::bullet("Push commits to the stack: ca push");
    Output::bullet("Submit for review: ca submit");
    Output::bullet("Check status: ca status");
    println!();
    Output::tip("Learn more:");
    Output::bullet("Run 'ca --help' for all commands");
    Output::bullet("Run 'ca doctor' to verify your setup");
    Output::bullet("Use 'ca --verbose <command>' for debug logging");
    Output::bullet("Run 'ca hooks status' to check hook installation");
    Output::bullet(
        "Configure PR templates: ca config set cascade.pr_description_template \"template\"",
    );
    Output::bullet("Visit docs/HOOKS.md for hook details");
    Output::bullet("Visit the documentation for advanced usage");

    Ok(())
}

/// Configure Git user settings (name and email)
async fn configure_git_user(git_repo: &GitRepository) -> Result<()> {
    let theme = ColorfulTheme::default();

    // Check current git configuration
    let repo_path = git_repo.path();
    let git_repo_inner = git2::Repository::open(repo_path)
        .map_err(|e| CascadeError::config(format!("Could not open git repository: {e}")))?;

    let mut current_name: Option<String> = None;
    let mut current_email: Option<String> = None;

    if let Ok(config) = git_repo_inner.config() {
        // Check for user configuration
        if let Ok(name) = config.get_string("user.name") {
            if !name.trim().is_empty() {
                current_name = Some(name);
            }
        }

        if let Ok(email) = config.get_string("user.email") {
            if !email.trim().is_empty() {
                current_email = Some(email);
            }
        }
    }

    // Display current configuration status
    match (&current_name, &current_email) {
        (Some(name), Some(email)) => {
            Output::success("Git user configuration found:");
            Output::sub_item(format!("Name: {name}"));
            Output::sub_item(format!("Email: {email}"));

            let keep_current = Confirm::with_theme(&theme)
                .with_prompt("Keep current Git user settings?")
                .default(true)
                .interact()
                .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

            if keep_current {
                Output::success("Using existing Git user configuration");
                return Ok(());
            }
        }
        _ => {
            if current_name.is_some() || current_email.is_some() {
                Output::warning("Git user configuration incomplete:");
                if let Some(name) = &current_name {
                    Output::sub_item(format!("Name: {name}"));
                } else {
                    Output::sub_item("Name: not configured");
                }
                if let Some(email) = &current_email {
                    Output::sub_item(format!("Email: {email}"));
                } else {
                    Output::sub_item("Email: not configured");
                }
            } else {
                Output::warning("Git user not configured");
                Output::info(
                    "Git user name and email are required for commits and Cascade operations",
                );
            }
        }
    }

    // Prompt for user information
    println!("\n👤 Git User Configuration");
    println!("   This information will be used for all git commits and Cascade operations.");

    let name: String = Input::with_theme(&theme)
        .with_prompt("Your name")
        .with_initial_text(current_name.unwrap_or_default())
        .validate_with(|input: &String| -> std::result::Result<(), &str> {
            if input.trim().is_empty() {
                Err("Name cannot be empty")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

    let email: String = Input::with_theme(&theme)
        .with_prompt("Your email")
        .with_initial_text(current_email.unwrap_or_default())
        .validate_with(|input: &String| -> std::result::Result<(), &str> {
            if input.trim().is_empty() {
                Err("Email cannot be empty")
            } else if !input.contains('@') {
                Err("Please enter a valid email address")
            } else {
                Ok(())
            }
        })
        .interact_text()
        .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

    // Ask about scope (global vs local)
    let use_global = Confirm::with_theme(&theme)
        .with_prompt("Set globally for all Git repositories? (otherwise only for this repository)")
        .default(true)
        .interact()
        .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

    // Set the configuration using git commands for reliability
    let scope_flag = if use_global { "--global" } else { "--local" };

    // Set user.name
    let output = std::process::Command::new("git")
        .args(["config", scope_flag, "user.name", &name])
        .current_dir(repo_path)
        .output()
        .map_err(|e| CascadeError::config(format!("Failed to execute git config: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CascadeError::config(format!(
            "Failed to set git user.name: {stderr}"
        )));
    }

    // Set user.email
    let output = std::process::Command::new("git")
        .args(["config", scope_flag, "user.email", &email])
        .current_dir(repo_path)
        .output()
        .map_err(|e| CascadeError::config(format!("Failed to execute git config: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CascadeError::config(format!(
            "Failed to set git user.email: {stderr}"
        )));
    }

    // Validate the configuration was set correctly
    match git_repo.validate_git_user_config() {
        Ok(_) => {
            Output::success("Git user configuration updated successfully!");
            if use_global {
                Output::sub_item("Configuration applied globally for all Git repositories");
            } else {
                Output::sub_item("Configuration applied to this repository only");
            }
            Output::sub_item(format!("Name: {name}"));
            Output::sub_item(format!("Email: {email}"));
        }
        Err(e) => {
            Output::warning(format!("Configuration set but validation failed: {e}"));
            Output::tip("You may need to check your git configuration manually");
        }
    }

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
                let base_url = format!("https://{host}");
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
        .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

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
        .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

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
        .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

    // Authentication token
    println!("\n🔐 Authentication Setup");
    println!("   Cascade needs a Personal Access Token to interact with Bitbucket.");
    println!("   You can create one at: {url}/plugins/servlet/access-tokens/manage");
    println!("   Required permissions: Repository Read, Repository Write");

    let configure_token = Confirm::with_theme(&theme)
        .with_prompt("Configure authentication token now?")
        .default(true)
        .interact()
        .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

    let token = if configure_token {
        let token: String = Input::with_theme(&theme)
            .with_prompt("Personal Access Token")
            .interact_text()
            .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

        if token.trim().is_empty() {
            None
        } else {
            Some(token.trim().to_string())
        }
    } else {
        Output::tip("You can configure the token later with:");
        Output::command_example("ca config set bitbucket.token YOUR_TOKEN");
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
            "Failed to connect to Bitbucket: {e}"
        ))),
    }
}

/// Configure PR description template interactively
async fn configure_pr_template(config_path: &std::path::Path) -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("   Configure a markdown template for PR descriptions.");
    println!("   This template will be used for ALL PRs (overrides --description).");
    println!("   You can use markdown formatting, variables, etc.");
    println!("   ");
    println!("   Example template:");
    println!("   ## Summary");
    println!("   Brief description of changes");
    println!("   ");
    println!("   ## Testing");
    println!("   - [ ] Unit tests pass");
    println!("   - [ ] Manual testing completed");

    let use_example = Confirm::with_theme(&theme)
        .with_prompt("Use the example template above?")
        .default(true)
        .interact()
        .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

    let template = if use_example {
        Some("## Summary\nBrief description of changes\n\n## Testing\n- [ ] Unit tests pass\n- [ ] Manual testing completed\n\n## Checklist\n- [ ] Code review completed\n- [ ] Documentation updated".to_string())
    } else {
        let custom_template: String = Input::with_theme(&theme)
            .with_prompt("Enter your PR description template (use \\n for line breaks)")
            .allow_empty(true)
            .interact_text()
            .map_err(|e| CascadeError::config(format!("Input error: {e}")))?;

        if custom_template.trim().is_empty() {
            None
        } else {
            // Replace literal \n with actual newlines
            Some(custom_template.replace("\\n", "\n"))
        }
    };

    // Load and update settings
    let mut settings = Settings::load_from_file(config_path)?;
    settings.cascade.pr_description_template = template;
    settings.save_to_file(config_path)?;

    if settings.cascade.pr_description_template.is_some() {
        Output::success("PR description template configured!");
        Output::tip("This template will be used for ALL future PRs");
        Output::tip(
            "Edit later with: ca config set cascade.pr_description_template \"Your template\"",
        );
    } else {
        Output::success("No template configured (will use --description or commit messages)");
    }

    Ok(())
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
