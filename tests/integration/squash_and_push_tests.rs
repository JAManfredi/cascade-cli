// Integration tests for squash and push functionality
// These tests focus on the logic WITHOUT making network requests

use cascade_cli::git::GitRepository;
use cascade_cli::stack::{StackManager, Stack, StackEntry};
use std::path::PathBuf;
use tempfile::TempDir;
use std::process::Command;

/// Test squash message generation with basic commits
#[tokio::test]
async fn test_squash_commits_basic() {
    let (_temp_dir, repo_path) = create_test_git_repo_with_commits().await;
    
    let messages = vec![
        "WIP: start feature".to_string(),
        "WIP: continue work".to_string(), 
        "Final: complete feature".to_string()
    ];
    
    let result = cascade_cli::cli::commands::stack::generate_squash_message(&messages, "final");
    assert!(result.is_ok());
    
    let message = result.unwrap();
    // Should extract meaningful content and avoid WIP messages
    assert!(message.contains("complete feature") || message.contains("Complete feature"));
    assert!(!message.contains("WIP"));
}

/// Test squash with WIP message detection
#[tokio::test]
async fn test_squash_with_wip_messages() {
    let messages = vec![
        "WIP: implement authentication".to_string(),
        "WIP: add validation".to_string(),
        "Fix: handle edge cases".to_string()
    ];
    
    let result = cascade_cli::cli::commands::stack::generate_squash_message(&messages, "final");
    assert!(result.is_ok());
    
    let message = result.unwrap();
    // Should extract feature from WIP messages
    assert!(message.contains("implement") || message.contains("Implement"));
}

/// Test different squash message strategies
#[tokio::test]
async fn test_generate_squash_message_strategies() {
    let messages = vec![
        "WIP: start user authentication".to_string(),
        "WIP: add validation logic".to_string(),
        "Final: implement user auth system".to_string()
    ];
    
    // Test "final" strategy
    let result = cascade_cli::cli::commands::stack::generate_squash_message(&messages, "final");
    assert!(result.is_ok());
    
    let message = result.unwrap();
    // Should extract feature from WIP messages
    assert!(message.contains("implement") || message.contains("Implement"));
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
    cascade_cli::config::initialize_repo(&repo_path, Some("https://test.bitbucket.com".to_string())).unwrap();
    
    // Create stack manager
    let mut stack_manager = StackManager::new(&repo_path).unwrap();
    
    // Create a stack
    let stack_id = stack_manager.create_stack("test-stack".to_string(), None).unwrap();
    
    // Verify stack was created
    let stacks = stack_manager.list_stacks().unwrap();
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].name, "test-stack");
    
    // Test stack operations
    let stack = stack_manager.get_stack(&stack_id).unwrap();
    assert_eq!(stack.name, "test-stack");
    assert_eq!(stack.entries.len(), 0);
}

/// Test range parsing for push/submit commands (no network)
#[tokio::test]
async fn test_range_parsing() {
    // Test valid range formats
    let ranges = vec!["1-3", "1,3,5", "2-5", "1"];
    
    for range in ranges {
        let result = cascade_cli::cli::commands::stack::parse_range_or_commits(range);
        assert!(result.is_ok(), "Range '{}' should be valid", range);
    }
    
    // Test invalid range formats  
    let invalid_ranges = vec!["", "a-b", "1-", "-3"];
    
    for range in invalid_ranges {
        let result = cascade_cli::cli::commands::stack::parse_range_or_commits(range);
        assert!(result.is_err(), "Range '{}' should be invalid", range);
    }
}

/// Test feature extraction from WIP messages (core logic)
#[tokio::test]
async fn test_extract_feature_from_wip() {
    let test_cases = vec![
        ("WIP: implement user authentication", "implement user authentication"),
        ("WIP: add validation logic", "add validation logic"),
        ("wip: fix bug in parser", "fix bug in parser"),
        ("Final: complete feature", "complete feature"),
        ("Regular commit message", "Regular commit message"),
    ];
    
    for (input, expected) in test_cases {
        let result = cascade_cli::cli::commands::stack::extract_feature_from_wip_commit(input);
        assert!(result.contains(expected) || result.to_lowercase().contains(&expected.to_lowercase()),
                "Input '{}' should extract '{}'", input, expected);
    }
}

// Helper functions for test setup

async fn create_test_git_repo() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    Command::new("git").args(&["init"]).current_dir(&repo_path).output().unwrap();
    Command::new("git").args(&["config", "user.name", "Test"]).current_dir(&repo_path).output().unwrap();
    Command::new("git").args(&["config", "user.email", "test@test.com"]).current_dir(&repo_path).output().unwrap();

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
    std::fs::write(&file_path, format!("Content for {}\n", filename)).unwrap();
    
    Command::new("git").args(&["add", filename]).current_dir(repo_path).output().unwrap();
    Command::new("git").args(&["commit", "-m", message]).current_dir(repo_path).output().unwrap();
}

async fn get_current_commit_hash(repo_path: &PathBuf) -> String {
    let output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    
    String::from_utf8_lossy(&output.stdout).trim().to_string()
} 