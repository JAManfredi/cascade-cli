/// Integration tests for `ca sync continue` and `ca sync abort` commands
/// These tests ensure conflict recovery works correctly during sync operations
use std::path::{Path, PathBuf};
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

/// Helper to create a stack with a file that will conflict
fn create_conflicting_stack(
    repo_path: &Path,
    stack_name: &str,
    base_branch: &str,
) -> Result<(), String> {
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

    // Create a commit with conflict.txt
    std::fs::write(repo_path.join("conflict.txt"), "Stack content").map_err(|e| e.to_string())?;
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to add: {e}"))?;
    Command::new("git")
        .args(["commit", "-m", "Add conflict file"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to commit: {e}"))?;

    let (success, _, stderr) =
        run_ca_command(&["push", "--allow-base-branch", "--yes"], repo_path)?;
    if !success {
        return Err(format!("Failed to push commit: {stderr}"));
    }

    // Now create conflicting change on base branch
    Command::new("git")
        .args(["checkout", base_branch])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to checkout base: {e}"))?;

    std::fs::write(repo_path.join("conflict.txt"), "Base branch content")
        .map_err(|e| e.to_string())?;
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to add: {e}"))?;
    Command::new("git")
        .args(["commit", "-m", "Conflicting change on base"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to commit: {e}"))?;

    // Switch back to stack
    run_ca_command(&["switch", stack_name], repo_path)?;

    Ok(())
}

/// Check if there's an in-progress cherry-pick
fn has_cherry_pick_in_progress(repo_path: &Path) -> bool {
    repo_path.join(".git/CHERRY_PICK_HEAD").exists()
}

/// Check if CASCADE_SYNC_STATE file exists
fn has_sync_state(repo_path: &Path) -> bool {
    repo_path.join(".git/CASCADE_SYNC_STATE").exists()
}

#[test]
fn test_sync_continue_after_manual_conflict_resolution() {
    if !ca_binary_exists() {
        eprintln!("Skipping test: ca binary not found");
        return;
    }

    let (temp_dir, base_branch) = setup_test_repo().unwrap();
    let repo_path = temp_dir.path();

    // Create a stack with conflicting changes
    create_conflicting_stack(repo_path, "conflict-stack", &base_branch).unwrap();

    // Run sync - may auto-resolve or fail with conflicts
    let (success, stdout, stderr) = run_ca_command(&["sync", "--force"], repo_path).unwrap();

    // If sync succeeded with auto-resolve, skip this test
    if success && stdout.contains("Sync completed successfully") {
        eprintln!("Skipping test: conflicts were auto-resolved");
        return;
    }

    // Otherwise, should have conflicts
    let has_conflict_message = stdout.contains("conflict")
        || stderr.contains("conflict")
        || stdout.contains("Conflict")
        || stderr.contains("Conflict");

    assert!(
        has_conflict_message,
        "Sync should report conflicts if not auto-resolved. stdout: {}, stderr: {}",
        stdout, stderr
    );

    // Should have cherry-pick in progress
    if has_cherry_pick_in_progress(repo_path) {
        // Manually resolve the conflict
        std::fs::write(repo_path.join("conflict.txt"), "Resolved content").unwrap();
        Command::new("git")
            .args(["add", "conflict.txt"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Run sync continue
        let (success, stdout, stderr) = run_ca_command(&["sync", "continue"], repo_path).unwrap();

        assert!(
            success,
            "sync continue should succeed after resolving conflicts. stdout: {}, stderr: {}",
            stdout, stderr
        );

        // Should not have cherry-pick in progress anymore
        assert!(
            !has_cherry_pick_in_progress(repo_path),
            "Should not have cherry-pick in progress after continue"
        );

        // Should not have sync state file
        assert!(
            !has_sync_state(repo_path),
            "Should not have sync state file after successful continue"
        );
    }
}

#[test]
fn test_sync_abort_cleans_up_state() {
    if !ca_binary_exists() {
        eprintln!("Skipping test: ca binary not found");
        return;
    }

    let (temp_dir, base_branch) = setup_test_repo().unwrap();
    let repo_path = temp_dir.path();

    // Create a stack with conflicting changes
    create_conflicting_stack(repo_path, "abort-stack", &base_branch).unwrap();

    // Run sync - may auto-resolve or fail with conflicts
    let (success, stdout, stderr) = run_ca_command(&["sync", "--force"], repo_path).unwrap();

    // If sync succeeded with auto-resolve, skip this test
    if success && stdout.contains("Sync completed successfully") {
        eprintln!("Skipping test: conflicts were auto-resolved");
        return;
    }

    // Otherwise, should have conflicts
    let has_conflict_message = stdout.contains("conflict")
        || stderr.contains("conflict")
        || stdout.contains("Conflict")
        || stderr.contains("Conflict");

    assert!(
        has_conflict_message,
        "Sync should report conflicts if not auto-resolved"
    );

    // If we have a cherry-pick in progress, abort it
    if has_cherry_pick_in_progress(repo_path) {
        // Run sync abort
        let (success, stdout, stderr) = run_ca_command(&["sync", "abort"], repo_path).unwrap();

        assert!(
            success,
            "sync abort should succeed. stdout: {}, stderr: {}",
            stdout, stderr
        );

        // Should not have cherry-pick in progress anymore
        assert!(
            !has_cherry_pick_in_progress(repo_path),
            "Should not have cherry-pick in progress after abort"
        );

        // Should not have sync state file
        assert!(
            !has_sync_state(repo_path),
            "Should not have sync state file after abort"
        );

        // Should be back on original branch (not a temp branch)
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

        assert!(
            !current_branch.contains("-temp-"),
            "Should not be on a temp branch after abort, got: {}",
            current_branch
        );
    }
}

#[test]
fn test_sync_continue_without_conflicts_fails() {
    if !ca_binary_exists() {
        eprintln!("Skipping test: ca binary not found");
        return;
    }

    let (temp_dir, _) = setup_test_repo().unwrap();
    let repo_path = temp_dir.path();

    // Try to run sync continue without any in-progress operation
    let (success, stdout, stderr) = run_ca_command(&["sync", "continue"], repo_path).unwrap();

    // Should fail because there's no cherry-pick in progress
    assert!(
        !success,
        "sync continue should fail when there's no cherry-pick in progress. stdout: {}, stderr: {}",
        stdout, stderr
    );

    // Error message should mention no cherry-pick
    let error_mentions_no_cherry_pick = stdout.contains("cherry-pick")
        || stderr.contains("cherry-pick")
        || stdout.contains("in-progress")
        || stderr.contains("in-progress");

    assert!(
        error_mentions_no_cherry_pick,
        "Error should mention no cherry-pick in progress"
    );
}

#[test]
fn test_sync_abort_without_conflicts_fails() {
    if !ca_binary_exists() {
        eprintln!("Skipping test: ca binary not found");
        return;
    }

    let (temp_dir, _) = setup_test_repo().unwrap();
    let repo_path = temp_dir.path();

    // Try to run sync abort without any in-progress operation
    let (success, stdout, stderr) = run_ca_command(&["sync", "abort"], repo_path).unwrap();

    // Should fail because there's no cherry-pick in progress
    assert!(
        !success,
        "sync abort should fail when there's no cherry-pick in progress. stdout: {}, stderr: {}",
        stdout, stderr
    );

    // Error message should mention no cherry-pick
    let error_mentions_no_cherry_pick = stdout.contains("cherry-pick")
        || stderr.contains("cherry-pick")
        || stdout.contains("in-progress")
        || stderr.contains("in-progress");

    assert!(
        error_mentions_no_cherry_pick,
        "Error should mention no cherry-pick in progress"
    );
}

#[test]
fn test_sync_state_persistence() {
    if !ca_binary_exists() {
        eprintln!("Skipping test: ca binary not found");
        return;
    }

    let (temp_dir, base_branch) = setup_test_repo().unwrap();
    let repo_path = temp_dir.path();

    // Create a stack with conflicting changes
    create_conflicting_stack(repo_path, "state-stack", &base_branch).unwrap();

    // Run sync - should create state file on conflict
    let (_success, _stdout, _stderr) = run_ca_command(&["sync", "--force"], repo_path).unwrap();

    // If we have a cherry-pick in progress, we should have a state file
    if has_cherry_pick_in_progress(repo_path) {
        assert!(
            has_sync_state(repo_path),
            "Should have sync state file when cherry-pick is in progress"
        );

        // Read the state file to verify it has correct structure
        let state_path = repo_path.join(".git/CASCADE_SYNC_STATE");
        let state_content = std::fs::read_to_string(&state_path).unwrap();

        // Should be valid JSON
        let state_json: serde_json::Value = serde_json::from_str(&state_content).unwrap();

        // Should have required fields
        assert!(
            state_json.get("stack_id").is_some(),
            "State should have stack_id"
        );
        assert!(
            state_json.get("stack_name").is_some(),
            "State should have stack_name"
        );
        assert!(
            state_json.get("original_branch").is_some(),
            "State should have original_branch"
        );
        assert!(
            state_json.get("target_base").is_some(),
            "State should have target_base"
        );
        assert!(
            state_json.get("temp_branches").is_some(),
            "State should have temp_branches"
        );
    }
}
