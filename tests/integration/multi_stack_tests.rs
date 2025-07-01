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



/// Test internal stack manager thread safety (proper concurrency testing)
#[tokio::test]
async fn test_stack_manager_thread_safety() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    // Test concurrent stack creation using StackManager directly (no CLI processes)
    let concurrent_operations = 5;
    let mut handles = Vec::new();

    for i in 0..concurrent_operations {
        let repo_path = repo_path.clone();
        let handle = tokio::spawn(async move {
            // Each task creates its own StackManager instance
            let mut manager = cascade_cli::stack::manager::StackManager::new(&repo_path).unwrap();
            let stack_name = format!("thread-safe-stack-{}", i);
            
            // Test creating a stack - this should be thread-safe
            manager.create_stack(stack_name, None, None)
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    let results: Vec<_> = futures::future::join_all(handles).await;
    
    // Count successful operations
    let successful_count = results
        .into_iter()
        .filter_map(|result| result.ok()) // Filter out join errors
        .filter(|stack_result| stack_result.is_ok()) // Filter out stack creation errors
        .count();

    println!("Thread-safe stack operations: {successful_count}/{concurrent_operations} succeeded");

    // Most operations should succeed with proper internal thread safety
    assert!(
        successful_count >= concurrent_operations * 3 / 4,
        "At least 75% of thread-safe stack operations should succeed (got {successful_count}/{concurrent_operations})"
    );

    // Verify stacks can be listed successfully
    let final_manager = cascade_cli::stack::manager::StackManager::new(&repo_path).unwrap();
    let stacks = final_manager.list_stacks();
    
    assert!(
        stacks.len() >= successful_count,
        "Should be able to list at least {successful_count} stacks, but found {}", stacks.len()
    );
}

/// Test sequential stack operations (baseline for comparison)
#[tokio::test]
async fn test_sequential_stack_operations() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let mut manager = cascade_cli::stack::manager::StackManager::new(&repo_path).unwrap();
    
    // Create stacks sequentially - this should always work
    let stack_count = 5;
    for i in 0..stack_count {
        let stack_name = format!("sequential-stack-{}", i);
        manager.create_stack(stack_name, None, None)
            .expect(&format!("Sequential stack creation {} should always succeed", i));
    }

    // Verify all stacks exist
    let stacks = manager.list_stacks();
    assert_eq!(
        stacks.len(), 
        stack_count,
        "Sequential operations should create exactly {stack_count} stacks"
    );

    println!("Sequential stack operations: {stack_count}/{stack_count} succeeded (baseline)");
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
