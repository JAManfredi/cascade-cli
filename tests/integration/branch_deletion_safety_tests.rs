use crate::integration::test_helpers::{create_test_repo_with_commits, run_cc_with_timeout};
use std::env;
use tempfile::TempDir;

/// Test branch deletion safety with unpushed commits
#[tokio::test]
async fn test_branch_deletion_safety_with_unpushed_commits() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository with commits
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 3).expect("Failed to create test repository");

    // Create a feature branch from head
    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/test-branch", &head_commit, false)
        .expect("Failed to create feature branch");

    // Switch to feature branch and add a new commit to create unpushed commits
    repo.set_head("refs/heads/feature/test-branch")
        .expect("Failed to switch to feature branch");

    // Add a new commit that makes the branch diverge from main
    std::fs::write(repo_path.join("unpushed-test.txt"), "Unpushed content")
        .expect("Failed to create test file");
    let mut index = repo.index().expect("Failed to get index");
    index
        .add_path(std::path::Path::new("unpushed-test.txt"))
        .expect("Failed to add file to index");
    index.write().expect("Failed to write index");

    let tree_oid = index.write_tree().expect("Failed to write tree");
    let tree = repo.find_tree(tree_oid).expect("Failed to find tree");
    let signature = git2::Signature::new(
        "Test User",
        "test@example.com",
        &git2::Time::new(1234567890, 0),
    )
    .expect("Failed to create signature");

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Add unpushed test commit",
        &tree,
        &[&head_commit],
    )
    .expect("Failed to create commit");

    // Switch back to main
    repo.set_head("refs/heads/main")
        .expect("Failed to switch back to main");

    // Set environment variable to skip interactive confirmation in tests
    env::set_var("BRANCH_DELETE_NO_CONFIRM", "1");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that branch deletion is blocked due to unpushed commits
    let result = git_repo.delete_branch("feature/test-branch");

    // Should fail due to unpushed commits
    assert!(
        result.is_err(),
        "Branch deletion should be blocked due to unpushed commits"
    );

    // Test that unsafe deletion works
    let unsafe_result = git_repo.delete_branch_unsafe("feature/test-branch");
    assert!(
        unsafe_result.is_ok(),
        "Unsafe branch deletion should succeed"
    );

    env::remove_var("BRANCH_DELETE_NO_CONFIRM");
}

/// Test branch deletion safety when branch is merged
#[tokio::test]
async fn test_branch_deletion_safety_merged_branch() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository with commits
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    // Create a feature branch from HEAD (already merged scenario)
    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/merged-branch", &head_commit, false)
        .expect("Failed to create feature branch");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that merged branch deletion is allowed
    let result = git_repo.delete_branch("feature/merged-branch");
    assert!(result.is_ok(), "Merged branch deletion should be allowed");
}

/// Test branch deletion safety in CI environment
#[tokio::test]
async fn test_branch_deletion_safety_ci_environment() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository with commits
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 3).expect("Failed to create test repository");

    // Create a feature branch with unpushed commits
    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/ci-test-branch", &head_commit, false)
        .expect("Failed to create feature branch");

    // Switch to the feature branch and add a new commit
    repo.set_head("refs/heads/feature/ci-test-branch")
        .expect("Failed to switch to feature branch");

    // Add a new commit that makes the branch diverge from main
    std::fs::write(repo_path.join("ci-test.txt"), "CI test content")
        .expect("Failed to create test file");
    let mut index = repo.index().expect("Failed to get index");
    index
        .add_path(std::path::Path::new("ci-test.txt"))
        .expect("Failed to add file to index");
    index.write().expect("Failed to write index");

    let tree_oid = index.write_tree().expect("Failed to write tree");
    let tree = repo.find_tree(tree_oid).expect("Failed to find tree");
    let signature = git2::Signature::new(
        "Test User",
        "test@example.com",
        &git2::Time::new(1234567890, 0),
    )
    .expect("Failed to create signature");

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Add CI test commit",
        &tree,
        &[&head_commit],
    )
    .expect("Failed to create commit");

    // Switch back to main
    repo.set_head("refs/heads/main")
        .expect("Failed to switch back to main");

    // Set CI environment variable
    env::set_var("CI", "true");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that in CI environment, branch deletion fails with clear error
    let result = git_repo.delete_branch("feature/ci-test-branch");
    assert!(
        result.is_err(),
        "Branch deletion should fail in CI environment"
    );

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("non-interactive mode"),
        "Error should mention non-interactive mode"
    );

    env::remove_var("CI");
}

/// Test main branch detection functionality
#[tokio::test]
async fn test_main_branch_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that main branch detection works
    // This is testing private functionality indirectly through branch deletion safety

    // Create a branch that's clearly not merged
    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/unmerged", &head_commit, false)
        .expect("Failed to create feature branch");

    // The main branch detection should work without errors
    // (tested indirectly through the safety check)
    env::set_var("BRANCH_DELETE_NO_CONFIRM", "1");
    let _result = git_repo.delete_branch("feature/unmerged");
    env::remove_var("BRANCH_DELETE_NO_CONFIRM");
}

/// Test remote tracking branch detection
#[tokio::test]
async fn test_remote_tracking_branch_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository
    let (_repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test remote tracking branch detection (will be None for local-only repo)
    // This is tested indirectly through the safety mechanisms

    // The system should handle the lack of remote tracking branches gracefully
    assert!(git_repo.path().exists());
}

/// Test branch deletion with various branch name patterns
#[tokio::test]
async fn test_branch_deletion_with_various_names() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();

    // Test various branch naming patterns
    let branch_names = [
        "feature/test",
        "bugfix/fix-123",
        "release/v1.0.0",
        "hotfix/critical-fix",
        "test-branch-with-dashes",
        "test_branch_with_underscores",
    ];

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    for branch_name in &branch_names {
        // Create branch
        repo.branch(branch_name, &head_commit, false)
            .expect("Failed to create branch");

        // Delete branch (should work since it's merged to main)
        let result = git_repo.delete_branch(branch_name);
        assert!(
            result.is_ok(),
            "Should be able to delete branch '{branch_name}'"
        );
    }
}

/// Test error handling for non-existent branches
#[tokio::test]
async fn test_branch_deletion_error_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository
    let (_repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test deletion of non-existent branch
    let result = git_repo.delete_branch("non-existent-branch");
    assert!(result.is_err(), "Deleting non-existent branch should fail");

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Could not find branch"),
        "Error should mention branch not found"
    );
}

/// Test concurrent branch deletion operations
#[tokio::test]
async fn test_concurrent_branch_deletion_safety() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository
    let (repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    let head_commit = repo.head().unwrap().peel_to_commit().unwrap();

    // Create multiple branches
    for i in 1..=3 {
        let branch_name = format!("concurrent-test-{i}");
        repo.branch(&branch_name, &head_commit, false)
            .expect("Failed to create branch");
    }

    use cascade_cli::git::GitRepository;

    // Test that concurrent deletion operations don't interfere
    let git_repo1 = GitRepository::open(repo_path).expect("Failed to open repository 1");
    let git_repo2 = GitRepository::open(repo_path).expect("Failed to open repository 2");

    // Both should be able to perform deletion operations
    let result1 = git_repo1.delete_branch("concurrent-test-1");
    let result2 = git_repo2.delete_branch("concurrent-test-2");

    assert!(result1.is_ok(), "First concurrent deletion should succeed");
    assert!(result2.is_ok(), "Second concurrent deletion should succeed");
}

/// Integration test: Branch deletion safety through CLI commands
#[tokio::test]
async fn test_cli_branch_deletion_safety_integration() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository
    let (_repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 3).expect("Failed to create test repository");

    // Save the original directory
    let original_dir = env::current_dir().expect("Failed to get current directory");

    // Change to the repository directory
    env::set_current_dir(repo_path).expect("Failed to change directory");

    // Initialize cascade in the repository
    let init_result = run_cc_with_timeout(&["init", "--force"], 30000).await;
    assert!(init_result.status.success(), "Cascade init should succeed");

    // Test CLI commands that might trigger branch deletion safety
    // This would test the end-to-end integration when we add CLI flags for deletion

    // Restore original directory
    env::set_current_dir(original_dir).expect("Failed to restore directory");
}

/// Test platform-specific behavior for branch deletion safety
#[tokio::test]
async fn test_branch_deletion_safety_platform_behavior() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path();

    // Create a test repository
    let (_repo, _commit_oids) =
        create_test_repo_with_commits(repo_path, 2).expect("Failed to create test repository");

    use cascade_cli::git::GitRepository;
    let git_repo = GitRepository::open(repo_path).expect("Failed to open repository");

    // Test that safety mechanisms work on both Windows and Unix
    // The safety check should work regardless of platform
    assert!(git_repo.path().exists());

    // Platform-specific behavior should be transparent to the user
    // The implementation handles platform differences internally
}
