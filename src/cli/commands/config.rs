use crate::cli::ConfigAction;
use crate::config::{get_repo_config_dir, is_repo_initialized, Settings};
use crate::errors::{CascadeError, Result};
use crate::git::find_repository_root;
use std::env;

/// Handle configuration commands
pub async fn run(action: ConfigAction) -> Result<()> {
    // Find the repository root
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)?;

    // Check if repository is initialized
    if !is_repo_initialized(&repo_root) {
        return Err(CascadeError::not_initialized(
            "Repository is not initialized for Cascade. Run 'ca init' first.",
        ));
    }

    let config_dir = get_repo_config_dir(&repo_root)?;
    let config_file = config_dir.join("config.json");

    match action {
        ConfigAction::Set { key, value } => set_config_value(&config_file, &key, &value).await,
        ConfigAction::Get { key } => get_config_value(&config_file, &key).await,
        ConfigAction::List => list_config_values(&config_file).await,
        ConfigAction::Unset { key } => unset_config_value(&config_file, &key).await,
    }
}

async fn set_config_value(config_file: &std::path::Path, key: &str, value: &str) -> Result<()> {
    let mut settings = Settings::load_from_file(config_file)?;
    settings.set_value(key, value)?;
    settings.validate()?;
    settings.save_to_file(config_file)?;

    println!("✅ Configuration updated: {key} = {value}");

    // Provide contextual hints
    match key {
        "bitbucket.token" => {
            println!("💡 Tip: You can create a personal access token in Bitbucket Server under:");
            println!("   Settings → Personal access tokens → Create token");
        }
        "bitbucket.url" => {
            println!("💡 Next: Set your project and repository:");
            println!("   ca config set bitbucket.project YOUR_PROJECT_KEY");
            println!("   ca config set bitbucket.repo your-repo-name");
        }
        "bitbucket.accept_invalid_certs" => {
            println!("💡 SSL Configuration:");
            if value == "true" {
                println!("   ⚠️  SSL certificate verification is disabled (development only)");
                println!("   This setting affects both API calls and git operations");
            } else {
                println!("   ✅ SSL certificate verification is enabled (recommended)");
                println!("   For custom CA certificates, use: ca config set bitbucket.ca_bundle_path /path/to/ca-bundle.crt");
            }
        }
        "bitbucket.ca_bundle_path" => {
            println!("💡 SSL Configuration:");
            println!("   📁 Custom CA bundle path set for SSL certificate verification");
            println!("   This affects both API calls and git operations");
            println!("   Make sure the file exists and contains valid PEM certificates");
        }
        _ => {}
    }

    Ok(())
}

async fn get_config_value(config_file: &std::path::Path, key: &str) -> Result<()> {
    let settings = Settings::load_from_file(config_file)?;
    let value = settings.get_value(key)?;

    // Mask sensitive values
    let display_value = if key.contains("token") || key.contains("password") {
        if value.is_empty() {
            "(not set)".to_string()
        } else {
            format!("{}***", &value[..std::cmp::min(4, value.len())])
        }
    } else if value.is_empty() {
        "(not set)".to_string()
    } else {
        value
    };

    println!("{key} = {display_value}");
    Ok(())
}

async fn list_config_values(config_file: &std::path::Path) -> Result<()> {
    let settings = Settings::load_from_file(config_file)?;

    println!("📋 Cascade Configuration:");
    println!();

    // Bitbucket configuration
    println!("🔗 Bitbucket Server:");
    print_config_value(&settings, "  bitbucket.url")?;
    print_config_value(&settings, "  bitbucket.project")?;
    print_config_value(&settings, "  bitbucket.repo")?;
    print_config_value(&settings, "  bitbucket.token")?;
    println!();

    // Git configuration
    println!("📦 Git:");
    print_config_value(&settings, "  git.default_branch")?;
    print_config_value(&settings, "  git.author_name")?;
    print_config_value(&settings, "  git.author_email")?;
    print_config_value(&settings, "  git.auto_cleanup_merged")?;
    print_config_value(&settings, "  git.prefer_rebase")?;
    println!();

    // Cascade configuration
    println!("⚙️  Cascade:");
    print_config_value(&settings, "  cascade.api_port")?;
    print_config_value(&settings, "  cascade.auto_cleanup")?;
    print_config_value(&settings, "  cascade.default_sync_strategy")?;
    print_config_value(&settings, "  cascade.max_stack_size")?;
    print_config_value(&settings, "  cascade.enable_notifications")?;

    Ok(())
}

fn print_config_value(settings: &Settings, key: &str) -> Result<()> {
    let key_without_spaces = key.trim();
    let value = settings.get_value(key_without_spaces)?;

    // Mask sensitive values
    let display_value =
        if key_without_spaces.contains("token") || key_without_spaces.contains("password") {
            if value.is_empty() {
                "(not set)".to_string()
            } else {
                format!("{}***", &value[..std::cmp::min(4, value.len())])
            }
        } else if value.is_empty() {
            "(not set)".to_string()
        } else {
            value
        };

    println!("{key} = {display_value}");
    Ok(())
}

async fn unset_config_value(config_file: &std::path::Path, key: &str) -> Result<()> {
    let mut settings = Settings::load_from_file(config_file)?;

    // Set the value to empty string to "unset" it
    settings.set_value(key, "")?;
    settings.save_to_file(config_file)?;

    println!("✅ Configuration value unset: {key}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::initialize_repo;
    use git2::{Repository, Signature};
    use tempfile::TempDir;

    async fn create_initialized_repo() -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repository
        let repo = Repository::init(&repo_path).unwrap();

        // Create initial commit
        let signature = Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )
        .unwrap();

        // Initialize Cascade
        initialize_repo(&repo_path, None).unwrap();

        (temp_dir, repo_path)
    }

    #[tokio::test]
    async fn test_config_set_get() {
        let (_temp_dir, repo_path) = create_initialized_repo().await;

        // Test directly with config file instead of changing directories
        let config_dir = crate::config::get_repo_config_dir(&repo_path).unwrap();
        let config_file = config_dir.join("config.json");

        // Set a configuration value
        set_config_value(&config_file, "bitbucket.url", "https://test.bitbucket.com")
            .await
            .unwrap();

        // Get the configuration value
        get_config_value(&config_file, "bitbucket.url")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_config_list() {
        let (_temp_dir, repo_path) = create_initialized_repo().await;

        // Test directly with config file instead of changing directories
        let config_dir = crate::config::get_repo_config_dir(&repo_path).unwrap();
        let config_file = config_dir.join("config.json");

        // List all configuration values
        list_config_values(&config_file).await.unwrap();
    }
}
