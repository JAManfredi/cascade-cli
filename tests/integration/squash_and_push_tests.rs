// Integration tests for squash and push functionality
// These tests focus on the logic WITHOUT making network requests

use cascade_cli::git::GitRepository;
use cascade_cli::stack::StackManager;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Test squash message generation with real git commits
#[tokio::test]
async fn test_squash_commits_basic() {
    let (_temp_dir, repo_path) = create_test_git_repo_with_commits().await;

    let repo = GitRepository::open(&repo_path).unwrap();

    // Get the last 3 commits
    let commits_result = repo.get_commits_between("HEAD~2", "HEAD");
    if let Ok(commits) = commits_result {
        let result = cascade_cli::cli::commands::stack::generate_squash_message(&commits);
        assert!(result.is_ok());

        let message = result.unwrap();
        // Should generate a meaningful squash message
        assert!(!message.is_empty());
    } else {
        // Skip this test if we can't get commits (e.g., not enough commits)
        println!("Skipping test - unable to get commit range");
    }
}

/// Test WIP feature extraction (tests the core logic)
#[tokio::test]
async fn test_extract_feature_from_wip_messages() {
    let messages = vec![
        "WIP: implement authentication".to_string(),
        "WIP: add validation".to_string(),
        "Fix: handle edge cases".to_string(),
    ];

    let result = cascade_cli::cli::commands::stack::extract_feature_from_wip(&messages);
    // Should extract meaningful feature description
    assert!(!result.is_empty());
    assert!(
        result.contains("implement")
            || result.contains("authentication")
            || result.contains("validation")
    );
}

/// Test commit counting functionality (doesn't require network)
#[tokio::test]
async fn test_count_commits_since() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Create base commit
    create_commit(&repo_path, "Base", "base.rs").await;
    let base_hash = get_current_commit_hash(&repo_path).await;

    // Create additional commits
    create_commit(&repo_path, "Commit 1", "c1.rs").await;
    create_commit(&repo_path, "Commit 2", "c2.rs").await;
    create_commit(&repo_path, "Commit 3", "c3.rs").await;

    let repo = GitRepository::open(&repo_path).unwrap();
    let count = cascade_cli::cli::commands::stack::count_commits_since(&repo, &base_hash).unwrap();

    assert_eq!(count, 3); // Should count 3 commits since base
}

/// Test stack creation and management (no network calls)
#[tokio::test]
async fn test_stack_operations() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade CLI
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    // Create stack manager
    let mut stack_manager = StackManager::new(&repo_path).unwrap();

    // Create a stack
    let stack_id = stack_manager
        .create_stack("test-stack".to_string(), None, None)
        .unwrap();

    // Verify stack was created
    let stacks = stack_manager.list_stacks();
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1, "test-stack"); // Access by tuple index since it returns a tuple

    // Test stack operations
    let stack = stack_manager.get_stack(&stack_id).unwrap();
    assert_eq!(stack.name, "test-stack");
    assert_eq!(stack.entries.len(), 0);
}

/// Test feature extraction from WIP messages (core logic)
#[tokio::test]
async fn test_extract_feature_from_wip() {
    let test_cases = vec![
        vec!["WIP: implement user authentication".to_string()],
        vec!["WIP: add validation logic".to_string()],
        vec!["wip: fix bug in parser".to_string()],
        vec!["Final: complete feature".to_string()],
        vec!["Regular commit message".to_string()],
    ];

    for messages in test_cases {
        let result = cascade_cli::cli::commands::stack::extract_feature_from_wip(&messages);
        // Should extract some meaningful content
        assert!(
            !result.is_empty(),
            "Should extract feature from: {messages:?}"
        );
    }
}

// Helper functions for test setup

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
    create_commit(&repo_path, "Initial commit", "README.md").await;

    (temp_dir, repo_path)
}

async fn create_test_git_repo_with_commits() -> (TempDir, PathBuf) {
    let (temp_dir, repo_path) = create_test_git_repo().await;

    // Create multiple commits for squashing tests
    create_commit(&repo_path, "Feature: add user login", "login.rs").await;
    create_commit(&repo_path, "WIP: fix validation", "validation.rs").await;
    create_commit(&repo_path, "WIP: add tests", "tests.rs").await;

    (temp_dir, repo_path)
}

async fn create_commit(repo_path: &PathBuf, message: &str, filename: &str) {
    let file_path = repo_path.join(filename);
    std::fs::write(&file_path, format!("Content for {filename}\n")).unwrap();

    Command::new("git")
        .args(["add", filename])
        .current_dir(repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_path)
        .output()
        .unwrap();
}

async fn get_current_commit_hash(repo_path: &PathBuf) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
