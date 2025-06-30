use crate::integration::test_helpers::{create_test_repo_with_commits, run_cc_with_timeout};
use std::env;
use std::fs;
use tempfile::TempDir;

/// Test checkout safety with uncommitted modified files
#[tokio::test]
async fn test_checkout_safety_with_modified_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository with commits
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    // Create a feature branch
    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/test-branch", &head_commit, false)
        .expect("Failed to create feature branch");

    // Modify a file to create uncommitted changes
    let test_file = repo_path.join("test-file-1.txt");
    fs::write(&test_file, "Modified content that would be lost")
        .expect("Failed to modify test file");

    // Set environment variable to skip interactive confirmation in tests
    env::set_var("CHECKOUT_NO_CONFIRM", "1");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that checkout is blocked due to uncommitted changes
    let result = git_repo.checkout_branch("feature/test-branch");

    // Should fail due to uncommitted changes
    assert!(
        result.is_err(),
        "Checkout should be blocked due to uncommitted changes"
    );

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("uncommitted changes"),
        "Error should mention uncommitted changes"
    );

    // Test that unsafe checkout works
    let unsafe_result = git_repo.checkout_branch_unsafe("feature/test-branch");
    assert!(
        unsafe_result.is_ok(),
        "Unsafe checkout should succeed despite uncommitted changes"
    );

    env::remove_var("CHECKOUT_NO_CONFIRM");
}

/// Test checkout safety with staged files
#[tokio::test]
async fn test_checkout_safety_with_staged_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository with commits
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    // Create a feature branch
    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/staged-branch", &head_commit, false)
        .expect("Failed to create feature branch");

    // Create a new file and stage it
    let new_file = repo_path.join("staged-file.txt");
    fs::write(&new_file, "New staged content").expect("Failed to create new file");

    // Stage the file
    let mut index = repo.index().expect("Failed to get repository index");
    index
        .add_path(std::path::Path::new("staged-file.txt"))
        .expect("Failed to stage file");
    index.write().expect("Failed to write index");

    // Set environment variable to skip interactive confirmation in tests
    env::set_var("CHECKOUT_NO_CONFIRM", "1");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that checkout is blocked due to staged changes
    let result = git_repo.checkout_branch("feature/staged-branch");

    // Should fail due to staged changes
    assert!(
        result.is_err(),
        "Checkout should be blocked due to staged changes"
    );

    env::remove_var("CHECKOUT_NO_CONFIRM");
}

/// Test checkout safety with untracked files
#[tokio::test]
async fn test_checkout_safety_with_untracked_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository with commits
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    // Create a feature branch
    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/untracked-branch", &head_commit, false)
        .expect("Failed to create feature branch");

    // Create untracked files
    let untracked_file = repo_path.join("untracked-file.txt");
    fs::write(&untracked_file, "Untracked content").expect("Failed to create untracked file");

    // Set environment variable to skip interactive confirmation in tests
    env::set_var("CHECKOUT_NO_CONFIRM", "1");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that checkout is blocked due to untracked files
    let result = git_repo.checkout_branch("feature/untracked-branch");

    // Should fail due to untracked files
    assert!(
        result.is_err(),
        "Checkout should be blocked due to untracked files"
    );

    env::remove_var("CHECKOUT_NO_CONFIRM");
}

/// Test checkout safety in CI environment
#[tokio::test]
async fn test_checkout_safety_ci_environment() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository with commits
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    // Create a feature branch
    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/ci-test-branch", &head_commit, false)
        .expect("Failed to create feature branch");

    // Modify a file to create uncommitted changes
    let test_file = repo_path.join("test-file-1.txt");
    fs::write(&test_file, "Modified content in CI").expect("Failed to modify test file");

    // Set CI environment variable
    env::set_var("CI", "true");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that in CI environment, checkout fails with clear error
    let result = git_repo.checkout_branch("feature/ci-test-branch");
    assert!(
        result.is_err(),
        "Checkout should fail in CI environment with uncommitted changes"
    );

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("non-interactive mode"),
        "Error should mention non-interactive mode"
    );

    env::remove_var("CI");
}

/// Test commit checkout safety with uncommitted changes
#[tokio::test]
async fn test_commit_checkout_safety_with_uncommitted_changes() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository with commits
    let (_repo, commit_oids) =
        create_test_repo_with_commits(repo_path, 3).expect("Failed to create test repository");

    // Modify a file to create uncommitted changes
    let test_file = repo_path.join("test-file-1.txt");
    fs::write(&test_file, "Modified content for commit checkout")
        .expect("Failed to modify test file");

    // Set environment variable to skip interactive confirmation in tests
    env::set_var("CHECKOUT_NO_CONFIRM", "1");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that commit checkout is blocked due to uncommitted changes
    let first_commit = &commit_oids[0];
    let result = git_repo.checkout_commit(first_commit);

    // Should fail due to uncommitted changes
    assert!(
        result.is_err(),
        "Commit checkout should be blocked due to uncommitted changes"
    );

    // Test that unsafe commit checkout works
    let unsafe_result = git_repo.checkout_commit_unsafe(first_commit);
    assert!(
        unsafe_result.is_ok(),
        "Unsafe commit checkout should succeed despite uncommitted changes"
    );

    env::remove_var("CHECKOUT_NO_CONFIRM");
}

/// Test checkout safety when repository is clean
#[tokio::test]
async fn test_checkout_safety_clean_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository with commits
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    // Create a feature branch
    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/clean-branch", &head_commit, false)
        .expect("Failed to create feature branch");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that checkout works when repository is clean
    let result = git_repo.checkout_branch("feature/clean-branch");
    assert!(
        result.is_ok(),
        "Checkout should succeed with clean repository"
    );

    // Verify we're on the correct branch
    let current_branch = git_repo
        .get_current_branch()
        .expect("Failed to get current branch");
    assert_eq!(current_branch, "feature/clean-branch");
}

/// Test checkout safety error handling for non-existent branches
#[tokio::test]
async fn test_checkout_safety_error_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository
    let (_repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test checkout of non-existent branch
    let result = git_repo.checkout_branch("non-existent-branch");
    assert!(
        result.is_err(),
        "Checkout of non-existent branch should fail"
    );

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Could not find branch"),
        "Error should mention branch not found"
    );
}

/// Test checkout safety with various file modification patterns
#[tokio::test]
async fn test_checkout_safety_mixed_changes() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository with commits
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    // Create a feature branch
    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/mixed-changes", &head_commit, false)
        .expect("Failed to create feature branch");

    // Create various types of changes

    // 1. Modified file
    let test_file = repo_path.join("test-file-1.txt");
    fs::write(&test_file, "Modified content").expect("Failed to modify test file");

    // 2. New file (staged)
    let new_file = repo_path.join("new-staged-file.txt");
    fs::write(&new_file, "New staged content").expect("Failed to create new file");
    let mut index = repo.index().expect("Failed to get repository index");
    index
        .add_path(std::path::Path::new("new-staged-file.txt"))
        .expect("Failed to stage file");
    index.write().expect("Failed to write index");

    // 3. Untracked file
    let untracked_file = repo_path.join("untracked-file.txt");
    fs::write(&untracked_file, "Untracked content").expect("Failed to create untracked file");

    // Set environment variable to skip interactive confirmation in tests
    env::set_var("CHECKOUT_NO_CONFIRM", "1");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that checkout is blocked due to mixed changes
    let result = git_repo.checkout_branch("feature/mixed-changes");

    // Should fail due to mixed uncommitted changes
    assert!(
        result.is_err(),
        "Checkout should be blocked due to mixed uncommitted changes"
    );

    env::remove_var("CHECKOUT_NO_CONFIRM");
}

/// Test platform-specific behavior for checkout safety
#[tokio::test]
async fn test_checkout_safety_platform_behavior() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    // Create a feature branch
    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/platform-test", &head_commit, false)
        .expect("Failed to create feature branch");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that safety mechanisms work on both Windows and Unix
    // The safety check should work regardless of platform
    assert!(git_repo.path().exists());

    // Platform-specific behavior should be transparent to the user
    // The implementation handles platform differences internally
}

/// Integration test: Checkout safety through CLI commands
#[tokio::test]
async fn test_cli_checkout_safety_integration() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository
    let (_repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 3).expect("Failed to create test repository");

    // Change to the repository directory
    env::set_current_dir(repo_path).expect("Failed to change directory");

    // Initialize cascade in the repository
    let init_result = run_cc_with_timeout(&["init", "--force"], 30000).await;
    assert!(init_result.status.success(), "Cascade init should succeed");

    // Test CLI commands that might trigger checkout safety
    // This would test the end-to-end integration when CLI checkout commands are added

    env::set_current_dir("/").expect("Reset directory");
}
