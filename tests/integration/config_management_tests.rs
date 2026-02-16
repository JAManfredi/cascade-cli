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

/// Test concurrent config access on Unix systems (aggressive)
#[cfg(unix)]
#[tokio::test]
async fn test_concurrent_config_access_unix() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let binary_path = super::test_helpers::get_binary_path();

    // Unix systems should handle more aggressive concurrency
    let concurrent_operations = 5;

    let operations: Vec<Box<dyn FnOnce() -> Result<String, String> + Send>> = (0
        ..concurrent_operations)
        .map(|i| {
            let binary_path = binary_path.clone();
            let repo_path = repo_path.clone();
            let closure: Box<dyn FnOnce() -> Result<String, String> + Send> = Box::new(move || {
                let stack_name = format!("unix-concurrent-{}-{}", std::process::id(), i);

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
                    Err(format!(
                        "Failed to create stack: stderr={stderr}, stdout={stdout}"
                    ))
                }
            });
            closure
        })
        .collect();

    let results = super::test_helpers::run_parallel_operations(
        operations,
        "unix_concurrent_config_access".to_string(),
    )
    .await;

    // Unix should handle concurrent access well with our file locking
    let successful_count = results.iter().filter(|result| result.is_ok()).count();
    let failed_count = results.len() - successful_count;

    println!("Unix concurrent results: {successful_count} succeeded, {failed_count} failed");

    // On Unix, we expect most operations to succeed due to file locking
    assert!(
        successful_count >= concurrent_operations / 2,
        "Unix should handle at least half of concurrent operations successfully. Results: {results:#?}"
    );

    // Verify config integrity after concurrent operations
    let config_path = repo_path.join(".cascade").join("config.json");
    let config = Settings::load_from_file(&config_path);
    assert!(
        config.is_ok(),
        "Config should be valid after concurrent access: {:?}",
        config.err()
    );
}

/// Test concurrent config access on Windows systems (conservative)
#[cfg(windows)]
#[tokio::test]
async fn test_concurrent_config_access_windows() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let binary_path = super::test_helpers::get_binary_path();

    // Windows has stricter file locking - test more conservative concurrency
    let concurrent_operations = 3;

    let operations: Vec<Box<dyn FnOnce() -> Result<String, String> + Send>> = (0
        ..concurrent_operations)
        .map(|i| {
            let binary_path = binary_path.clone();
            let repo_path = repo_path.clone();
            let closure: Box<dyn FnOnce() -> Result<String, String> + Send> = Box::new(move || {
                let stack_name = format!("windows-concurrent-{}-{}", std::process::id(), i);

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

                    // On Windows, we might expect some operations to fail due to file locking
                    if stderr.contains("lock")
                        || stderr.contains("access")
                        || stderr.contains("timeout")
                    {
                        Err(format!("Expected Windows file locking behavior: {stderr}"))
                    } else {
                        Err(format!(
                            "Unexpected error: stderr={stderr}, stdout={stdout}"
                        ))
                    }
                }
            });
            closure
        })
        .collect();

    let results = super::test_helpers::run_parallel_operations(
        operations,
        "windows_concurrent_config_access".to_string(),
    )
    .await;

    let successful_count = results.iter().filter(|result| result.is_ok()).count();
    let expected_lock_failures = results
        .iter()
        .filter(|result| {
            if let Err(error) = result {
                error.contains("Expected Windows file locking behavior")
            } else {
                false
            }
        })
        .count();
    let unexpected_failures = results.len() - successful_count - expected_lock_failures;

    println!(
        "Windows concurrent results: {successful_count} succeeded, {expected_lock_failures} expected lock failures, {unexpected_failures} unexpected failures"
    );

    // On Windows, we expect either success or proper file locking failures
    assert!(
        unexpected_failures == 0,
        "Should not have unexpected failures on Windows. Results: {results:#?}"
    );

    // At least one operation should succeed
    assert!(
        successful_count > 0,
        "At least one operation should succeed on Windows"
    );

    // Verify config integrity after concurrent operations
    let config_path = repo_path.join(".cascade").join("config.json");
    let config = Settings::load_from_file(&config_path);
    assert!(
        config.is_ok(),
        "Config should be valid after concurrent access: {:?}",
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
    if let Ok(config) = config_result {
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

/// Test file locking implementation directly
#[tokio::test]
async fn test_file_locking_implementation() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;
    let test_file = repo_path.join("test_lock.json");

    // Test that file locking actually works
    let content1 = r#"{"test": "content1"}"#;
    let content2 = r#"{"test": "content2"}"#;

    // Write initial content
    cascade_cli::utils::atomic_file::write_string(&test_file, content1).unwrap();

    // Test concurrent writes with explicit locking
    let test_file_clone = test_file.clone();
    let handle1 = tokio::task::spawn_blocking(move || {
        // This should acquire the lock first
        cascade_cli::utils::atomic_file::write_string(&test_file_clone, content2)
    });

    let test_file_clone2 = test_file.clone();
    let handle2 = tokio::task::spawn_blocking(move || {
        // This should wait for the lock or fail appropriately
        std::thread::sleep(std::time::Duration::from_millis(100)); // Slight delay
        cascade_cli::utils::atomic_file::write_string(&test_file_clone2, content1)
    });

    let results = tokio::try_join!(handle1, handle2);

    match results {
        Ok((result1, result2)) => {
            // Both should either succeed (with proper ordering) or one should fail with timeout
            let success_count = [&result1, &result2].iter().filter(|r| r.is_ok()).count();
            let timeout_count = [&result1, &result2]
                .iter()
                .filter(|r| {
                    if let Err(e) = r {
                        e.to_string().contains("timeout") || e.to_string().contains("lock")
                    } else {
                        false
                    }
                })
                .count();

            println!("File locking test: {success_count} succeeded, {timeout_count} timed out");

            // At least one should succeed, and failures should be due to locking
            assert!(success_count > 0, "At least one write should succeed");
            assert!(
                success_count + timeout_count == 2,
                "All operations should either succeed or fail with locking errors"
            );
        }
        Err(e) => {
            panic!("Task execution failed: {e}");
        }
    }

    // File should still be valid JSON
    let final_content = std::fs::read_to_string(&test_file).unwrap();
    assert!(
        final_content == content1 || final_content == content2,
        "File should contain valid content after locking test"
    );
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
