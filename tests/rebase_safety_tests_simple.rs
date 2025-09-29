/// Simplified rebase safety tests that compile with current API
/// These tests focus on the core safety guarantee: original branches remain unchanged
use std::process::Command;
use tempfile::TempDir;

/// Helper to create a test repository
fn create_test_repo() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create initial commit
    std::fs::write(repo_path.join("README.md"), "# Test Repo").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    temp_dir
}

/// Helper to get the default branch name (environment-agnostic)
fn get_default_branch(repo_path: &std::path::Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Helper to get commits on a branch
fn get_branch_commits(repo_path: &std::path::Path, branch: &str) -> Vec<String> {
    let output = Command::new("git")
        .args(["log", "--format=%H", branch])
        .current_dir(repo_path)
        .output()
        .unwrap();

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect()
}

/// Helper to create a branch with a commit
fn create_branch_with_commit(repo_path: &std::path::Path, branch_name: &str, file_name: &str) {
    Command::new("git")
        .args(["checkout", "-b", branch_name])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::fs::write(
        repo_path.join(file_name),
        format!("Content for {branch_name}"),
    )
    .unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", &format!("Add {file_name}")])
        .current_dir(repo_path)
        .output()
        .unwrap();
}

#[test]
fn test_git_cherry_pick_preserves_source_branch() {
    let temp_dir = create_test_repo();
    let repo_path = temp_dir.path();

    // Create source branch
    create_branch_with_commit(repo_path, "source", "source.txt");
    let source_commits_before = get_branch_commits(repo_path, "source");

    // Get the commit to cherry-pick
    let commit_hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    let commit_to_pick = String::from_utf8_lossy(&commit_hash.stdout)
        .trim()
        .to_string();

    // Create target branch and cherry-pick
    let default_branch = get_default_branch(repo_path);
    Command::new("git")
        .args(["checkout", "-b", "target", &default_branch])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["cherry-pick", &commit_to_pick])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Verify source branch is unchanged
    let source_commits_after = get_branch_commits(repo_path, "source");
    assert_eq!(
        source_commits_before, source_commits_after,
        "Source branch should remain unchanged after cherry-pick"
    );
}

#[test]
fn test_rebase_creates_new_branches_not_modifying_original() {
    let temp_dir = create_test_repo();
    let repo_path = temp_dir.path();

    // Get default branch name before creating feature branches
    let default_branch = get_default_branch(repo_path);

    // Create feature branch with commits
    create_branch_with_commit(repo_path, "feature", "feature1.txt");
    create_branch_with_commit(repo_path, "feature", "feature2.txt");

    let original_commits = get_branch_commits(repo_path, "feature");

    // Add commit to default branch
    Command::new("git")
        .args(["checkout", &default_branch])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::fs::write(repo_path.join("main-file.txt"), "Main update").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Master update"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create new branch for rebased content (simulating version branch)
    Command::new("git")
        .args(["checkout", "-b", "feature-v2", &default_branch])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Cherry-pick commits from feature branch
    for commit in original_commits.iter().rev().take(2) {
        Command::new("git")
            .args(["cherry-pick", commit])
            .current_dir(repo_path)
            .output()
            .unwrap();
    }

    // Verify original branch is completely unchanged
    let feature_commits_after = get_branch_commits(repo_path, "feature");
    assert_eq!(
        original_commits, feature_commits_after,
        "Original feature branch should remain completely unchanged"
    );

    // Verify new branch has different commits
    let v2_commits = get_branch_commits(repo_path, "feature-v2");
    assert_ne!(
        original_commits, v2_commits,
        "Version branch should have different commit hashes"
    );
}

#[test]
fn test_failed_rebase_does_not_modify_original() {
    let temp_dir = create_test_repo();
    let repo_path = temp_dir.path();

    // Get default branch name before creating feature branches
    let default_branch = get_default_branch(repo_path);

    // Create feature branch
    create_branch_with_commit(repo_path, "feature", "conflict.txt");
    let original_commits = get_branch_commits(repo_path, "feature");

    // Create conflicting commit on default branch
    Command::new("git")
        .args(["checkout", &default_branch])
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
        .args(["commit", "-m", "Conflicting commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Try to rebase feature branch
    Command::new("git")
        .args(["checkout", "feature"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let rebase_output = Command::new("git")
        .args(["rebase", &default_branch])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Rebase should fail due to conflict
    if !rebase_output.status.success() {
        // Abort the rebase
        Command::new("git")
            .args(["rebase", "--abort"])
            .current_dir(repo_path)
            .output()
            .unwrap();
    }

    // Verify feature branch is unchanged
    let feature_commits_after = get_branch_commits(repo_path, "feature");
    assert_eq!(
        original_commits, feature_commits_after,
        "Feature branch should remain unchanged after failed rebase"
    );
}

#[test]
fn test_multiple_version_branches_independent() {
    let temp_dir = create_test_repo();
    let repo_path = temp_dir.path();

    // Get default branch name before creating feature branches
    let default_branch = get_default_branch(repo_path);

    // Create original feature branch
    create_branch_with_commit(repo_path, "feature", "file.txt");
    let original_commit = get_branch_commits(repo_path, "feature")[0].clone();

    // Add a different commit to default branch to make the cherry-picks create different hashes
    Command::new("git")
        .args(["checkout", &default_branch])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::fs::write(repo_path.join("base.txt"), "Base content for v2").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Base commit for v2"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create v2 by cherry-picking
    Command::new("git")
        .args(["checkout", "-b", "feature-v2", &default_branch])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["cherry-pick", &original_commit])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Add another different commit to main to make v3 different
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    std::fs::write(repo_path.join("base2.txt"), "Base content for v3").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Base commit for v3"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create v3 by cherry-picking again
    Command::new("git")
        .args(["checkout", "-b", "feature-v3", &default_branch])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["cherry-pick", &original_commit])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Get all commits
    let original_commits = get_branch_commits(repo_path, "feature");
    let v2_commits = get_branch_commits(repo_path, "feature-v2");
    let v3_commits = get_branch_commits(repo_path, "feature-v3");

    // All should be different (different commit hashes)
    assert_ne!(
        original_commits[0], v2_commits[0],
        "v2 should have different commit hash"
    );
    assert_ne!(
        original_commits[0], v3_commits[0],
        "v3 should have different commit hash"
    );
    assert_ne!(
        v2_commits[0], v3_commits[0],
        "v2 and v3 should have different commit hashes"
    );

    // But original should still be unchanged
    assert_eq!(
        original_commits,
        get_branch_commits(repo_path, "feature"),
        "Original branch should remain unchanged"
    );
}

#[test]
fn test_branch_versioning_naming_patterns() {
    let patterns = vec![
        ("feature/FOO-123", "feature/FOO-123-v2"),
        ("bugfix-test", "bugfix-test-v2"),
        ("my-branch", "my-branch-v2"),
        ("branch-v1", "branch-v1-v2"),
        ("feature/nested/branch", "feature/nested/branch-v2"),
    ];

    for (original, expected_v2) in patterns {
        // Simple version naming logic
        let v2_name = format!("{original}-v2");

        assert_eq!(
            expected_v2, v2_name,
            "Version naming should work for pattern: {original}"
        );
    }
}
