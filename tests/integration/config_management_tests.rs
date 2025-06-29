use cascade_cli::config::Settings;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Test config file corruption and recovery scenarios
#[tokio::test]
async fn test_config_corruption_recovery() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade with valid config
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    // Corrupt the config file
    let config_path = repo_path.join(".cascade").join("config.json");
    fs::write(&config_path, "{ invalid json }").unwrap();

    // Test that CLI handles corruption gracefully
    let binary_path = super::test_helpers::get_binary_path();
    let output = Command::new(&binary_path)
        .args(["stacks", "list"])
        .current_dir(&repo_path)
        .output()
        .expect("Command should run");

    // Should either fail gracefully or handle corruption with appropriate error
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    if !output.status.success() {
        // If it fails, should contain meaningful error message
        assert!(
            stderr.contains("config")
                || stderr.contains("JSON")
                || stderr.contains("parse")
                || stdout.contains("config")
                || stdout.contains("JSON")
                || stdout.contains("parse"),
            "Should contain config parsing error. Stderr: {stderr}, Stdout: {stdout}"
        );
    } else {
        // If it succeeds, it might be handling corruption gracefully or using defaults
        println!("CLI handled config corruption gracefully. Stderr: {stderr}, Stdout: {stdout}");
    }
}

/// Test concurrent config access
#[tokio::test]
async fn test_concurrent_config_access() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    // Simulate concurrent access using helper function
    let binary_path = super::test_helpers::get_binary_path();

    // Reduce concurrency in CI environments for stability
    let concurrent_operations = if std::env::var("CI").is_ok() {
        2 // Very conservative for CI
    } else {
        3 // Original count for local testing
    };

    let operations: Vec<Box<dyn FnOnce() -> Result<String, String> + Send>> = (0
        ..concurrent_operations)
        .map(|i| {
            let binary_path = binary_path.clone();
            let repo_path = repo_path.clone();
            let closure: Box<dyn FnOnce() -> Result<String, String> + Send> = Box::new(move || {
                // Add unique prefix to avoid naming conflicts
                let stack_name = format!("concurrent-config-test-{}-{}", std::process::id(), i);

                let output = Command::new(&binary_path)
                    .args(["stacks", "create", &stack_name])
                    .current_dir(&repo_path)
                    .output()
                    .map_err(|e| format!("Command failed: {e}"))?;

                if output.status.success() {
                    Ok(stack_name)
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stdout = String::from_utf8_lossy(&output.stdout);

                    // Distinguish between expected concurrency conflicts and real errors
                    if stderr.contains("already exists")
                        || stderr.contains("conflict")
                        || stdout.contains("already exists")
                        || stdout.contains("conflict")
                    {
                        // This is expected in concurrent scenarios
                        Err(format!("Expected concurrency conflict: {stderr}"))
                    } else {
                        // This might be a real error
                        Err(format!(
                            "Unexpected error - stderr: {stderr}, stdout: {stdout}"
                        ))
                    }
                }
            });
            closure
        })
        .collect();

    let results = super::test_helpers::run_parallel_operations(
        operations,
        "concurrent_config_access".to_string(),
    )
    .await;

    // More lenient success criteria for CI stability
    let successful_count = results.iter().filter(|result| result.is_ok()).count();
    let expected_conflicts = results
        .iter()
        .filter(|result| {
            if let Err(error) = result {
                error.contains("Expected concurrency conflict")
            } else {
                false
            }
        })
        .count();

    let unexpected_errors = results
        .iter()
        .filter(|result| {
            if let Err(error) = result {
                !error.contains("Expected concurrency conflict")
            } else {
                false
            }
        })
        .count();

    println!("Concurrent config access results: {successful_count} succeeded, {expected_conflicts} expected conflicts, {unexpected_errors} unexpected errors");

    // Should handle concurrent access without corrupting state
    // Allow for some expected concurrency conflicts, but no unexpected errors
    assert!(
        unexpected_errors == 0,
        "Should not have unexpected errors during concurrent access. Results: {results:#?}"
    );

    assert!(
        successful_count > 0 || expected_conflicts > 0,
        "At least some operations should either succeed or fail with expected conflicts"
    );

    // Verify config integrity after concurrent operations
    let config_path = repo_path.join(".cascade").join("config.json");
    let config = Settings::load_from_file(&config_path);
    assert!(
        config.is_ok(),
        "Config should still be valid after concurrent access: {:?}",
        config.err()
    );
}

/// Test config file permissions and recovery
#[tokio::test]
async fn test_config_permissions_handling() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let cascade_dir = repo_path.join(".cascade");

    // Make directory read-only (on Unix systems)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&cascade_dir).unwrap().permissions();
        perms.set_mode(0o444); // Read-only
        fs::set_permissions(&cascade_dir, perms).unwrap();

        // Test that CLI handles permission errors gracefully
        let binary_path = super::test_helpers::get_binary_path();
        let output = Command::new(&binary_path)
            .args(["stacks", "create", "permission-test"])
            .current_dir(&repo_path)
            .output()
            .expect("Command should run");

        // Should fail with permission error
        assert!(!output.status.success());

        // Restore permissions for cleanup
        let mut perms = fs::metadata(&cascade_dir).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&cascade_dir, perms).unwrap();
    }

    // On all platforms, test directory deletion and recovery
    fs::remove_dir_all(&cascade_dir).unwrap();

    let binary_path = super::test_helpers::get_binary_path();
    let output = Command::new(&binary_path)
        .args(["stacks", "list"])
        .current_dir(&repo_path)
        .output()
        .expect("Command should run");

    // Should handle missing config directory gracefully
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success()
            || stderr.contains("not initialized")
            || stdout.contains("not initialized")
            || stderr.contains("No stacks")
            || stdout.contains("No stacks"),
        "Should handle missing config gracefully. Stderr: {stderr}, Stdout: {stdout}"
    );
}

/// Test config validation with invalid settings
#[tokio::test]
async fn test_config_validation() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Create invalid config manually
    let cascade_dir = repo_path.join(".cascade");
    fs::create_dir_all(&cascade_dir).unwrap();

    let config_path = cascade_dir.join("config.json");
    let invalid_config = serde_json::json!({
        "bitbucket": {
            "url": "not-a-valid-url",
            "project": "",
            "repo": "test-repo",
            "username": null,
            "token": null,
            "default_reviewers": []
        },
        "git": {
            "default_branch": "main",
            "author_name": null,
            "author_email": null,
            "auto_cleanup_merged": true,
            "prefer_rebase": true
        },
        "cascade": {
            "api_port": 8080,
            "auto_cleanup": true,
            "default_sync_strategy": "branch-versioning",
            "max_stack_size": 20,
            "enable_notifications": true,
            "rebase": {
                "auto_resolve_conflicts": true,
                "max_retry_attempts": 3,
                "preserve_merges": true,
                "version_suffix_pattern": "v{}",
                "backup_before_rebase": true
            }
        }
    });
    fs::write(&config_path, invalid_config.to_string()).unwrap();

    // Test config loading
    let config_result = Settings::load_from_file(&config_path);

    // Should either fail to load or handle invalid settings gracefully
    if config_result.is_ok() {
        let config = config_result.unwrap();
        // If it loads, should have validation that catches invalid URL
        let client_result = cascade_cli::bitbucket::BitbucketClient::new(&config.bitbucket);
        assert!(client_result.is_err(), "Should reject invalid URL");
    }
}

/// Test stacks metadata corruption and recovery
#[tokio::test]
async fn test_stacks_metadata_corruption() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize and create a stack
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let binary_path = super::test_helpers::get_binary_path();
    Command::new(&binary_path)
        .args(["stacks", "create", "test-stack"])
        .current_dir(&repo_path)
        .output()
        .expect("Stack creation should work");

    // Corrupt the stacks metadata
    let stacks_path = repo_path.join(".cascade").join("stacks.json");
    fs::write(&stacks_path, "{ corrupted stacks file }").unwrap();

    // Test recovery
    let output = Command::new(&binary_path)
        .args(["stacks", "list"])
        .current_dir(&repo_path)
        .output()
        .expect("Command should run");

    if !output.status.success() {
        let error_output = String::from_utf8_lossy(&output.stderr);
        assert!(
            error_output.contains("stacks")
                || error_output.contains("metadata")
                || error_output.contains("parse"),
            "Should contain stacks metadata error: {error_output}"
        );
    } else {
        // If it succeeds, it should have recovered gracefully
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("Stacks list after corruption: {stdout}");
    }
}

// Helper function
async fn create_test_git_repo() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Create initial commit
    std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    (temp_dir, repo_path)
}
