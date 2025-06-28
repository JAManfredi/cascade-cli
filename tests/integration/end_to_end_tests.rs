use std::process::Command;
use tempfile::TempDir;
use std::path::PathBuf;

/// Test complete stack workflow from creation to PR submission
#[tokio::test]
async fn test_complete_stack_workflow() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;
    
    // Test stack creation
    let output = Command::new("cargo")
        .args(&["run", "--", "stack", "create", "test-feature"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to create stack");
    
    assert!(output.status.success());
    
    // Make test commits
    create_test_commits(&repo_path, 3).await;
    
    // Test stack push
    let output = Command::new("cargo")
        .args(&["run", "--", "stack", "push", "--all"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to push stack");
    
    assert!(output.status.success());
    
    // Test stack list
    let output = Command::new("cargo")
        .args(&["run", "--", "stack", "list"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to list stacks");
    
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("test-feature"));
}

/// Test rebase scenarios with conflict resolution
#[tokio::test]
async fn test_rebase_conflict_scenarios() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;
    
    // Create conflicting changes on main
    create_conflicting_base_changes(&repo_path).await;
    
    // Create stack with conflicting commits
    create_conflicting_stack(&repo_path).await;
    
    // Test automatic conflict resolution
    let output = Command::new("cargo")
        .args(&["run", "--", "stack", "rebase", "--strategy", "three-way-merge"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to rebase with conflicts");
    
    // Should handle resolvable conflicts automatically
    assert!(output.status.success());
}

/// Test CLI error handling and recovery scenarios
#[tokio::test]
async fn test_error_recovery_scenarios() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;
    
    // Test with invalid repository
    let invalid_path = repo_path.join("nonexistent");
    let output = Command::new("cargo")
        .args(&["run", "--", "stack", "create", "test"])
        .current_dir(&invalid_path)
        .output()
        .expect("Command should fail gracefully");
    
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not a git repository"));
}

async fn create_test_git_repo() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();
    
    // Initialize git repository with proper setup
    Command::new("git").args(&["init"]).current_dir(&repo_path).output().unwrap();
    Command::new("git").args(&["config", "user.name", "Test"]).current_dir(&repo_path).output().unwrap();
    Command::new("git").args(&["config", "user.email", "test@test.com"]).current_dir(&repo_path).output().unwrap();
    
    // Create initial commit
    std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
    Command::new("git").args(&["add", "."]).current_dir(&repo_path).output().unwrap();
    Command::new("git").args(&["commit", "-m", "Initial"]).current_dir(&repo_path).output().unwrap();
    
    // Initialize Cascade
    cascade_cli::config::initialize_repo(&repo_path, Some("https://test.bitbucket.com".to_string())).unwrap();
    
    (temp_dir, repo_path)
}

async fn create_test_commits(repo_path: &PathBuf, count: u32) {
    for i in 1..=count {
        std::fs::write(repo_path.join(format!("file{}.txt", i)), format!("Content {}", i)).unwrap();
        Command::new("git").args(&["add", "."]).current_dir(repo_path).output().unwrap();
        Command::new("git").args(&["commit", "-m", &format!("Add file {}", i)]).current_dir(repo_path).output().unwrap();
    }
}

async fn create_conflicting_base_changes(repo_path: &PathBuf) {
    std::fs::write(repo_path.join("shared.txt"), "Base content").unwrap();
    Command::new("git").args(&["add", "."]).current_dir(repo_path).output().unwrap();
    Command::new("git").args(&["commit", "-m", "Add shared file"]).current_dir(repo_path).output().unwrap();
}

async fn create_conflicting_stack(repo_path: &PathBuf) {
    // Create stack that modifies the same file
    Command::new("cargo").args(&["run", "--", "stack", "create", "conflict-test"]).current_dir(repo_path).output().unwrap();
    
    std::fs::write(repo_path.join("shared.txt"), "Stack content").unwrap();
    Command::new("git").args(&["add", "."]).current_dir(repo_path).output().unwrap();
    Command::new("git").args(&["commit", "-m", "Modify shared file"]).current_dir(repo_path).output().unwrap();
} 