use std::path::{Path, PathBuf};
/// Integration tests for the `ca sync` command
/// These tests ensure that sync operations work correctly and don't corrupt branches
use std::process::{Command, Stdio};
use tempfile::TempDir;

// Include the test helpers module
#[path = "integration/test_helpers.rs"]
mod test_helpers;

// Get the path to the cascade CLI binary
fn get_binary_path() -> PathBuf {
    test_helpers::get_binary_path()
}

// Skip tests if ca binary is not available
fn ca_binary_exists() -> bool {
    let binary_path = get_binary_path();
    binary_path.exists()
}

/// Helper to run ca command and get output
fn run_ca_command(args: &[&str], cwd: &Path) -> Result<(bool, String, String), String> {
    let binary_path = get_binary_path();
    let output = Command::new(&binary_path)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to execute ca: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok((output.status.success(), stdout, stderr))
}

/// Helper to create test repository with cascade initialized
fn setup_test_repo() -> Result<(TempDir, String), String> {
    let temp_dir = TempDir::new().map_err(|e| e.to_string())?;
    let repo_path = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to init git: {e}"))?;

    // Configure git
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to set user.name: {e}"))?;

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to set user.email: {e}"))?;

    // Create initial commit
    std::fs::write(repo_path.join("README.md"), "# Test Repo").map_err(|e| e.to_string())?;

    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to add files: {e}"))?;

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to commit: {e}"))?;

    // Rename to main
    Command::new("git")
        .args(["branch", "-m", "master", "main"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to rename branch: {e}"))?;

    // Initialize cascade
    let (success, _, _) = run_ca_command(&["init"], repo_path)?;
    if !success {
        return Err("Failed to initialize cascade".to_string());
    }

    Ok((temp_dir, "main".to_string()))
}

/// Helper to create a stack with commits
fn create_test_stack(repo_path: &Path, stack_name: &str) -> Result<(), String> {
    // Create stack
    let (success, _, stderr) = run_ca_command(&["stacks", "create", stack_name], repo_path)?;
    if !success {
        return Err(format!("Failed to create stack: {stderr}"));
    }

    // Switch to the stack
    let (success, _, stderr) = run_ca_command(&["stacks", "switch", stack_name], repo_path)?;
    if !success {
        return Err(format!("Failed to switch to stack: {stderr}"));
    }

    // Create first commit
    std::fs::write(repo_path.join("file1.txt"), "Content 1").map_err(|e| e.to_string())?;
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to add: {e}"))?;

    let (success, _, stderr) = run_ca_command(
        &["push", "--allow-base-branch", "-m", "First commit"],
        repo_path,
    )?;
    if !success {
        return Err(format!("Failed to push first commit: {stderr}"));
    }

    // Create second commit
    std::fs::write(repo_path.join("file2.txt"), "Content 2").map_err(|e| e.to_string())?;
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to add: {e}"))?;

    let (success, _, stderr) = run_ca_command(
        &["push", "--allow-base-branch", "-m", "Second commit"],
        repo_path,
    )?;
    if !success {
        return Err(format!("Failed to push second commit: {stderr}"));
    }

    Ok(())
}

/// Get commits on current branch
fn get_current_branch_commits(repo_path: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args(["log", "--format=%H %s", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect()
}

/// Get current branch name
fn get_current_branch(repo_path: &Path) -> String {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn test_sync_basic_functionality() {
    if !ca_binary_exists() {
        eprintln!("Skipping test: ca binary not found");
        return;
    }
    let (temp_dir, _) = setup_test_repo().unwrap();
    let repo_path = temp_dir.path();

    // Create a stack
    create_test_stack(repo_path, "test-stack").unwrap();

    // Get original branch state
    let original_branch = get_current_branch(repo_path);
    let _original_commits = get_current_branch_commits(repo_path);

    // Run sync
    let (success, stdout, stderr) = run_ca_command(&["sync", "--force"], repo_path).unwrap();
    assert!(success, "Sync should succeed: {stderr}");
    assert!(
        stdout.contains("Sync completed") || stdout.contains("up to date"),
        "Should show sync status"
    );

    // Should be on a new version branch
    let new_branch = get_current_branch(repo_path);
    assert!(
        new_branch.contains("-v2") || new_branch == original_branch,
        "Should be on version branch or same branch if no changes"
    );
}

#[test]
fn test_sync_with_upstream_changes() {
    let (temp_dir, base_branch) = setup_test_repo().unwrap();
    let repo_path = temp_dir.path();

    // Create a stack
    create_test_stack(repo_path, "feature-stack").unwrap();

    // Simulate upstream changes on main
    Command::new("git")
        .args(["checkout", &base_branch])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::fs::write(repo_path.join("upstream.txt"), "Upstream change").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Upstream change"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Switch back to stack
    run_ca_command(&["switch", "feature-stack"], repo_path).unwrap();

    // Get pre-sync state
    let _pre_sync_commits = get_current_branch_commits(repo_path);

    // Run sync
    let (success, _stdout, stderr) = run_ca_command(&["sync", "--force"], repo_path).unwrap();
    assert!(
        success,
        "Sync should succeed with upstream changes: {stderr}",
    );

    // Sync may switch back to main branch depending on implementation
    let post_sync_branch = get_current_branch(repo_path);

    // The sync operation may leave us on main, feature-stack, or create a version branch
    assert!(
        post_sync_branch == "main"
            || post_sync_branch == "feature-stack"
            || post_sync_branch.contains("-v2"),
        "Should be on main, original branch, or version branch after sync, got: '{post_sync_branch}'"
    );

    // New branch should have the upstream changes
    let post_sync_commits = get_current_branch_commits(repo_path);
    assert!(
        post_sync_commits
            .iter()
            .any(|c| c.contains("Upstream change")),
        "Should include upstream changes"
    );
}

#[test]
fn test_sync_preserves_original_branches() {
    let (temp_dir, _) = setup_test_repo().unwrap();
    let repo_path = temp_dir.path();

    // Create a stack
    create_test_stack(repo_path, "preserve-test").unwrap();

    // Get all branches before sync
    let output = Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let branches_before: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    // Get commits on original branches
    let mut original_commits = std::collections::HashMap::new();
    for branch in &branches_before {
        if branch != "main" && !branch.contains("-v") {
            let commits = Command::new("git")
                .args(["log", "--format=%H", branch])
                .current_dir(repo_path)
                .output()
                .unwrap();

            original_commits.insert(
                branch.clone(),
                String::from_utf8_lossy(&commits.stdout).to_string(),
            );
        }
    }

    // Run sync
    let (success, _, stderr) = run_ca_command(&["sync", "--force"], repo_path).unwrap();
    assert!(success, "Sync should succeed: {stderr}");

    // Verify original branches still have same commits
    for (branch, original) in original_commits {
        let commits = Command::new("git")
            .args(["log", "--format=%H", &branch])
            .current_dir(repo_path)
            .output()
            .unwrap();

        let current = String::from_utf8_lossy(&commits.stdout).to_string();
        assert_eq!(
            original, current,
            "Branch {branch} should have unchanged commits after sync"
        );
    }
}

#[test]
fn test_sync_with_conflicts() {
    let (temp_dir, base_branch) = setup_test_repo().unwrap();
    let repo_path = temp_dir.path();

    // Create a stack with a file
    run_ca_command(&["stacks", "create", "conflict-stack"], repo_path).unwrap();
    run_ca_command(&["stacks", "switch", "conflict-stack"], repo_path).unwrap();

    std::fs::write(repo_path.join("conflict.txt"), "Original content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    run_ca_command(
        &["push", "--allow-base-branch", "-m", "Add conflict file"],
        repo_path,
    )
    .unwrap();

    // Create conflicting change on main
    Command::new("git")
        .args(["checkout", &base_branch])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::fs::write(repo_path.join("conflict.txt"), "Conflicting content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Conflicting change"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Switch back and sync
    run_ca_command(&["switch", "conflict-stack"], repo_path).unwrap();

    // Sync should handle conflicts gracefully (either auto-resolve or report for manual resolution)
    let (success, stdout, stderr) = run_ca_command(&["sync", "--force"], repo_path).unwrap();

    // Should either:
    // 1. Auto-resolve and succeed (shows "Sync completed successfully" or "Auto-resolved")
    // 2. Report conflicts for manual resolution (contains "conflict")
    let handled_gracefully = success 
        && (stdout.to_lowercase().contains("sync completed") 
            || stdout.to_lowercase().contains("auto-resolved")
            || stdout.to_lowercase().contains("conflict")
            || stderr.to_lowercase().contains("conflict"));

    assert!(
        handled_gracefully,
        "Sync should handle conflicts gracefully (auto-resolve or report). Got stdout: {}, stderr: {}", 
        stdout, stderr
    );
}

#[test]
fn test_sync_force_push_option() {
    let (temp_dir, _) = setup_test_repo().unwrap();
    let repo_path = temp_dir.path();

    // Create a stack
    create_test_stack(repo_path, "force-test").unwrap();

    // Run sync with force flag
    let (success, stdout, _) = run_ca_command(&["sync", "--force"], repo_path).unwrap();
    assert!(success, "Sync with --force should succeed");

    // Check that it mentions force push in output
    assert!(
        stdout.contains("force") || stdout.contains("Force"),
        "Should mention force push in output"
    );
}

#[test]
fn test_sync_empty_stack() {
    let (temp_dir, _) = setup_test_repo().unwrap();
    let repo_path = temp_dir.path();

    // Create empty stack
    run_ca_command(&["stacks", "create", "empty-stack"], repo_path).unwrap();
    run_ca_command(&["stacks", "switch", "empty-stack"], repo_path).unwrap();

    // Sync should handle empty stack gracefully
    let (success, stdout, _) = run_ca_command(&["sync", "--force"], repo_path).unwrap();
    assert!(success, "Sync should succeed on empty stack");
    assert!(
        stdout.contains("empty") || stdout.contains("no entries") || stdout.contains("up to date"),
        "Should indicate stack is empty or up to date"
    );
}

#[test]
fn test_sync_multiple_stacks() {
    let (temp_dir, _) = setup_test_repo().unwrap();
    let repo_path = temp_dir.path();

    // Create first stack
    create_test_stack(repo_path, "stack-1").unwrap();
    let stack1_branch = get_current_branch(repo_path);

    // Create second stack
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    create_test_stack(repo_path, "stack-2").unwrap();
    let stack2_branch = get_current_branch(repo_path);

    // Sync first stack
    run_ca_command(&["switch", "stack-1"], repo_path).unwrap();
    let (success1, _, _) = run_ca_command(&["sync", "--force"], repo_path).unwrap();
    assert!(success1, "Sync of stack-1 should succeed");

    // Sync second stack
    run_ca_command(&["switch", "stack-2"], repo_path).unwrap();
    let (success2, _, _) = run_ca_command(&["sync", "--force"], repo_path).unwrap();
    assert!(success2, "Sync of stack-2 should succeed");

    // Both stacks should remain independent
    let output = Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let all_branches = String::from_utf8_lossy(&output.stdout);
    assert!(
        all_branches.contains(&stack1_branch),
        "Stack 1 branches should exist"
    );
    assert!(
        all_branches.contains(&stack2_branch),
        "Stack 2 branches should exist"
    );
}
