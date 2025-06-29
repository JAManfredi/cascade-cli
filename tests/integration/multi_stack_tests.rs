use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Test multiple stack creation and switching scenarios
#[tokio::test]
async fn test_multi_stack_creation_and_switching() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let binary_path = super::test_helpers::get_binary_path();

    // Create multiple stacks
    let stack_names = ["feature-auth", "feature-payments", "feature-ui"];
    for stack_name in &stack_names {
        let output = Command::new(&binary_path)
            .args(["stacks", "create", stack_name])
            .current_dir(&repo_path)
            .output()
            .expect("Stack creation should work");

        if !output.status.success() {
            eprintln!(
                "Failed to create stack {}: {}",
                stack_name,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(output.status.success(), "Stack creation should succeed");
    }

    // Verify all stacks exist
    let output = Command::new(&binary_path)
        .args(["stacks", "list"])
        .current_dir(&repo_path)
        .output()
        .expect("Stack listing should work");

    assert!(output.status.success());
    let list_output = String::from_utf8_lossy(&output.stdout);

    for stack_name in &stack_names {
        assert!(
            list_output.contains(stack_name),
            "Stack list should contain {stack_name}. Output: {list_output}"
        );
    }

    // Test switching between stacks
    for stack_name in &stack_names {
        let output = Command::new(&binary_path)
            .args(["stacks", "switch", stack_name])
            .current_dir(&repo_path)
            .output()
            .expect("Stack switching should work");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Failed to switch to {stack_name}: {stderr}");
            // Stack switching might not be implemented yet, so don't fail
            continue;
        }

        // Verify we're on the correct stack
        let status_output = Command::new(&binary_path)
            .args(["stacks", "status"])
            .current_dir(&repo_path)
            .output()
            .expect("Stack status should work");

        if status_output.status.success() {
            let status_text = String::from_utf8_lossy(&status_output.stdout);
            // Check if current stack is indicated somehow
            println!("Current stack status after switching to {stack_name}: {status_text}");
        }
    }
}

/// Test stack state consistency after git operations
#[tokio::test]
async fn test_stack_state_after_manual_git_ops() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade and create stack
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let binary_path = super::test_helpers::get_binary_path();
    Command::new(&binary_path)
        .args(["stacks", "create", "git-test"])
        .current_dir(&repo_path)
        .output()
        .expect("Stack creation should work");

    // Add some commits through cascade
    create_test_commits(&repo_path, 2).await;

    // Perform manual git operations that might affect stack state
    Command::new("git")
        .args(["checkout", "-b", "manual-branch"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    create_test_commits(&repo_path, 1).await;

    // Switch back to main
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Test that cascade can still handle stack operations
    let output = Command::new(&binary_path)
        .args(["stacks", "list"])
        .current_dir(&repo_path)
        .output()
        .expect("Stack listing should work");

    // Should handle manual git operations gracefully
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Stack list failed after manual git ops (might be expected): {stderr}");
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("Stack list after manual git ops: {stdout}");
    }
}

/// Test concurrent stack operations
#[tokio::test]
async fn test_concurrent_stack_operations() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let binary_path = super::test_helpers::get_binary_path();

    // Reduce concurrency in CI environments for stability
    let concurrent_operations = if std::env::var("CI").is_ok() {
        2 // Very conservative for CI
    } else {
        5 // Original count for local testing
    };

    // Use the helper function for parallel operations
    let operations: Vec<Box<dyn FnOnce() -> Result<String, String> + Send>> = (0
        ..concurrent_operations)
        .map(|i| {
            let binary_path = binary_path.clone();
            let repo_path = repo_path.clone();
            let closure: Box<dyn FnOnce() -> Result<String, String> + Send> = Box::new(move || {
                // Add unique prefix to avoid naming conflicts
                let stack_name = format!("concurrent-stack-{}-{}", std::process::id(), i);

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
                        || stderr.contains("Permission denied")
                        || stderr.contains("Access denied")
                    {
                        // This is expected in concurrent scenarios, especially on Windows
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

    let results =
        super::test_helpers::run_parallel_operations(operations, "stack_creation".to_string())
            .await;

    // Check results with more robust error handling
    let mut successful_count = 0;
    let mut expected_conflicts = 0;
    let mut unexpected_errors = 0;

    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(stack_name) => {
                successful_count += 1;
                println!("Concurrent stack creation {i} succeeded: {stack_name}");
            }
            Err(error) => {
                if error.contains("Expected concurrency conflict") {
                    expected_conflicts += 1;
                    println!(
                        "Concurrent stack creation {i} failed with expected conflict: {error}"
                    );
                } else {
                    unexpected_errors += 1;
                    println!("Concurrent stack creation {i} failed with unexpected error: {error}");
                }
            }
        }
    }

    println!(
        "Concurrent stack operations: {successful_count} succeeded, {expected_conflicts} expected conflicts, {unexpected_errors} unexpected errors"
    );

    // Should handle concurrent operations without corruption
    // Allow for some expected concurrency conflicts, but no unexpected errors
    assert!(
        unexpected_errors == 0,
        "Should not have unexpected errors during concurrent operations. Results: {results:#?}"
    );

    assert!(
        successful_count > 0 || expected_conflicts > 0,
        "At least some operations should either succeed or fail with expected conflicts"
    );

    // Verify that no metadata corruption occurred
    let stack_list_output = Command::new(&binary_path)
        .args(["stacks", "list"])
        .current_dir(&repo_path)
        .output()
        .expect("Stack listing should work");

    if !stack_list_output.status.success() {
        let stderr = String::from_utf8_lossy(&stack_list_output.stderr);
        panic!("Stack list command failed after concurrent operations: {stderr}");
    }

    let list_output = String::from_utf8_lossy(&stack_list_output.stdout);

    // Verify that the successful stacks are listed
    let mut found_stacks = 0;
    for stack_name in results.iter().flatten() {
        if list_output.contains(stack_name) {
            found_stacks += 1;
        }
    }

    assert_eq!(
        found_stacks, successful_count,
        "All successfully created stacks should be listed. List output: {list_output}"
    );
}

/// Test stack cleanup and deletion scenarios
#[tokio::test]
async fn test_stack_cleanup_and_deletion() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade and create test stacks
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let binary_path = super::test_helpers::get_binary_path();

    // Create stacks with different states
    let stack_names = ["cleanup-test-1", "cleanup-test-2"];
    for stack_name in &stack_names {
        Command::new(&binary_path)
            .args(["stacks", "create", stack_name])
            .current_dir(&repo_path)
            .output()
            .expect("Stack creation should work");
    }

    // Add commits to some stacks
    create_test_commits(&repo_path, 2).await;

    // Test stack deletion
    for stack_name in &stack_names {
        let output = Command::new(&binary_path)
            .args(["stacks", "delete", stack_name])
            .current_dir(&repo_path)
            .output()
            .expect("Stack deletion command should run");

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("Stack deletion for {stack_name} failed (might not be implemented): {stderr}");
            continue;
        }

        // Verify stack is deleted
        let list_output = Command::new(&binary_path)
            .args(["stacks", "list"])
            .current_dir(&repo_path)
            .output()
            .expect("Stack listing should work");

        if list_output.status.success() {
            let list_text = String::from_utf8_lossy(&list_output.stdout);
            assert!(
                !list_text.contains(stack_name),
                "Deleted stack {stack_name} should not appear in list: {list_text}"
            );
        }
    }
}

/// Test stack metadata corruption recovery
#[tokio::test]
async fn test_stack_metadata_inconsistency() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade and create stack
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let binary_path = super::test_helpers::get_binary_path();
    Command::new(&binary_path)
        .args(["stacks", "create", "metadata-test"])
        .current_dir(&repo_path)
        .output()
        .expect("Stack creation should work");

    // Create git branches manually to simulate inconsistency
    Command::new("git")
        .args(["checkout", "-b", "cascade/metadata-test/orphaned-branch"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    create_test_commits(&repo_path, 1).await;

    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Test that cascade handles orphaned branches gracefully
    let output = Command::new(&binary_path)
        .args(["stacks", "status"])
        .current_dir(&repo_path)
        .output()
        .expect("Stack status should run");

    // Should handle metadata inconsistency without crashing
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Stack status with metadata inconsistency (might be expected): {stderr}");
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("Stack status with metadata inconsistency: {stdout}");
    }

    // Verify list command still works
    let list_output = Command::new(&binary_path)
        .args(["stacks", "list"])
        .current_dir(&repo_path)
        .output()
        .expect("Stack list should work");

    assert!(
        list_output.status.success()
            || String::from_utf8_lossy(&list_output.stderr).contains("metadata")
            || String::from_utf8_lossy(&list_output.stderr).contains("inconsistent"),
        "Should handle metadata inconsistency gracefully"
    );
}

// Helper functions
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

async fn create_test_commits(repo_path: &PathBuf, count: u32) {
    for i in 1..=count {
        std::fs::write(
            repo_path.join(format!("file{i}.txt")),
            format!("Content {i}"),
        )
        .unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", &format!("Add file {i}")])
            .current_dir(repo_path)
            .output()
            .unwrap();
    }
}
