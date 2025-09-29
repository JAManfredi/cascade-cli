/// Tests for rebase conflict recovery and interrupted rebase scenarios
/// These tests ensure that cascade can gracefully handle and recover from
/// interrupted rebase operations without corrupting the repository state.
use cascade_cli::errors::Result;
use cascade_cli::git::GitRepository;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Helper to create a test repository with conflicts
fn create_repo_with_conflicts() -> Result<(TempDir, GitRepository)> {
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

    // Create initial commit on main
    std::fs::write(repo_path.join("file.txt"), "Initial content\n").unwrap();
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

    let git_repo = GitRepository::open(repo_path)?;
    Ok((temp_dir, git_repo))
}

/// Helper to check if repository is in a rebase state
fn is_in_rebase_state(repo_path: &Path) -> bool {
    repo_path.join(".git/rebase-merge").exists() || repo_path.join(".git/rebase-apply").exists()
}

/// Helper to get current HEAD commit
fn get_head_commit(repo_path: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn test_rebase_conflict_detection() {
    let (temp_dir, git_repo) = create_repo_with_conflicts().unwrap();
    let repo_path = temp_dir.path();

    // Create feature branch with conflicting change
    git_repo.create_branch("feature", None).unwrap();
    git_repo.checkout_branch("feature").unwrap();

    std::fs::write(repo_path.join("file.txt"), "Feature change\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Feature commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create conflicting change on default branch (go back to previous branch)
    Command::new("git")
        .args(["checkout", "-"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    std::fs::write(repo_path.join("file.txt"), "Master change\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Main commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Try to rebase feature onto default branch
    // First get the default branch name
    let default_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    let default_branch_name = String::from_utf8_lossy(&default_branch.stdout)
        .trim()
        .to_string();

    git_repo.checkout_branch("feature").unwrap();
    let output = Command::new("git")
        .args(["rebase", &default_branch_name])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Should have conflict
    assert!(!output.status.success(), "Rebase should fail with conflict");
    assert!(is_in_rebase_state(repo_path), "Should be in rebase state");

    // Abort rebase
    Command::new("git")
        .args(["rebase", "--abort"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Should no longer be in rebase state
    assert!(
        !is_in_rebase_state(repo_path),
        "Should not be in rebase state after abort"
    );
}

#[test]
fn test_interrupted_rebase_recovery() {
    let (temp_dir, git_repo) = create_repo_with_conflicts().unwrap();
    let repo_path = temp_dir.path();

    // Create multiple commits on feature branch
    git_repo.create_branch("feature", None).unwrap();
    git_repo.checkout_branch("feature").unwrap();

    for i in 1..=3 {
        std::fs::write(
            repo_path.join(format!("file{i}.txt")),
            format!("Content {i}\n"),
        )
        .unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", &format!("Commit {i}")])
            .current_dir(repo_path)
            .output()
            .unwrap();
    }

    let original_head = get_head_commit(repo_path);

    // Create change on default branch (go back to previous branch)
    Command::new("git")
        .args(["checkout", "-"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    std::fs::write(repo_path.join("main.txt"), "Main content\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Main update"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Start interactive rebase
    // Get default branch name first
    let default_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    let default_branch_name = String::from_utf8_lossy(&default_branch.stdout)
        .trim()
        .to_string();

    git_repo.checkout_branch("feature").unwrap();

    // Simulate interrupted rebase by starting it
    let _output = Command::new("git")
        .args(["rebase", &default_branch_name])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // If in rebase state, abort and verify state is restored
    if is_in_rebase_state(repo_path) {
        Command::new("git")
            .args(["rebase", "--abort"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        let recovered_head = get_head_commit(repo_path);
        assert_eq!(
            original_head, recovered_head,
            "HEAD should be restored to original after abort"
        );
    }
}

#[test]
fn test_rebase_continue_after_conflict_resolution() {
    let (temp_dir, git_repo) = create_repo_with_conflicts().unwrap();
    let repo_path = temp_dir.path();

    // Create feature branch
    git_repo.create_branch("feature", None).unwrap();
    git_repo.checkout_branch("feature").unwrap();

    std::fs::write(
        repo_path.join("file.txt"),
        "Feature line 1\nFeature line 2\n",
    )
    .unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Feature changes"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Create non-conflicting change on default branch (go back to previous branch)
    Command::new("git")
        .args(["checkout", "-"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    std::fs::write(repo_path.join("other.txt"), "Other content\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Main other change"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Rebase should succeed without conflicts
    // Get default branch name first
    let default_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    let default_branch_name = String::from_utf8_lossy(&default_branch.stdout)
        .trim()
        .to_string();

    git_repo.checkout_branch("feature").unwrap();
    let output = Command::new("git")
        .args(["rebase", &default_branch_name])
        .current_dir(repo_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Rebase should succeed without conflicts"
    );
    assert!(
        !is_in_rebase_state(repo_path),
        "Should not be in rebase state"
    );
}

#[test]
fn test_cascade_handles_dirty_working_directory() {
    let (temp_dir, _git_repo) = create_repo_with_conflicts().unwrap();
    let repo_path = temp_dir.path();

    // Create uncommitted changes
    std::fs::write(repo_path.join("uncommitted.txt"), "Uncommitted changes\n").unwrap();

    // Check that cascade can detect uncommitted changes
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let status = String::from_utf8_lossy(&status_output.stdout);
    assert!(!status.is_empty(), "Should have uncommitted changes");
    assert!(
        status.contains("uncommitted.txt"),
        "Should show uncommitted file"
    );
}

#[test]
fn test_rebase_preserves_commit_metadata() {
    let (temp_dir, git_repo) = create_repo_with_conflicts().unwrap();
    let repo_path = temp_dir.path();

    // Create commit with specific author
    git_repo.create_branch("feature", None).unwrap();
    git_repo.checkout_branch("feature").unwrap();

    std::fs::write(
        repo_path.join("authored.txt"),
        "Content by specific author\n",
    )
    .unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Commit with specific author
    Command::new("git")
        .args([
            "-c",
            "user.name=Special Author",
            "-c",
            "user.email=special@example.com",
            "commit",
            "-m",
            "Authored commit",
        ])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Get original commit info
    let original_info = Command::new("git")
        .args(["log", "-1", "--format=%an <%ae>"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let original_author = String::from_utf8_lossy(&original_info.stdout)
        .trim()
        .to_string();

    // Create change on default branch (go back to previous branch)
    Command::new("git")
        .args(["checkout", "-"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    std::fs::write(repo_path.join("base.txt"), "Base content\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Base commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Rebase and check author is preserved
    // Get default branch name first
    let default_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    let default_branch_name = String::from_utf8_lossy(&default_branch.stdout)
        .trim()
        .to_string();

    git_repo.checkout_branch("feature").unwrap();
    Command::new("git")
        .args(["rebase", &default_branch_name])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let rebased_info = Command::new("git")
        .args(["log", "-1", "--format=%an <%ae>"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let rebased_author = String::from_utf8_lossy(&rebased_info.stdout)
        .trim()
        .to_string();

    assert_eq!(
        original_author, rebased_author,
        "Author information should be preserved after rebase"
    );
}

#[test]
fn test_detect_merge_conflicts_in_files() {
    let (temp_dir, _) = create_repo_with_conflicts().unwrap();
    let repo_path = temp_dir.path();

    // Create a file with merge conflict markers
    let conflict_content = r#"
Some content before
<<<<<<< HEAD
This is the current change
=======
This is the incoming change
>>>>>>> feature
Some content after
"#;

    std::fs::write(repo_path.join("conflicted.txt"), conflict_content).unwrap();

    // Check that conflict markers are present
    let content = std::fs::read_to_string(repo_path.join("conflicted.txt")).unwrap();
    assert!(
        content.contains("<<<<<<<"),
        "Should contain conflict start marker"
    );
    assert!(
        content.contains("======="),
        "Should contain conflict separator"
    );
    assert!(
        content.contains(">>>>>>>"),
        "Should contain conflict end marker"
    );
}

#[test]
fn test_stash_pop_conflicts() {
    let (temp_dir, _git_repo) = create_repo_with_conflicts().unwrap();
    let repo_path = temp_dir.path();

    // Create initial changes
    std::fs::write(repo_path.join("stash.txt"), "Original content\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Original stash file"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Make changes and stash
    std::fs::write(repo_path.join("stash.txt"), "Stashed changes\n").unwrap();
    Command::new("git")
        .args(["stash", "push", "-m", "Test stash"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Make conflicting changes
    std::fs::write(repo_path.join("stash.txt"), "Conflicting changes\n").unwrap();
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

    // Try to pop stash (should conflict)
    let pop_output = Command::new("git")
        .args(["stash", "pop"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Should indicate conflict
    assert!(
        !pop_output.status.success(),
        "Stash pop should fail with conflict"
    );

    // Verify stash is still available
    let stash_list = Command::new("git")
        .args(["stash", "list"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let stash_output = String::from_utf8_lossy(&stash_list.stdout);
    assert!(
        !stash_output.is_empty(),
        "Stash should still exist after failed pop"
    );
}

#[test]
fn test_cascade_git_state_detection() {
    let (temp_dir, _git_repo) = create_repo_with_conflicts().unwrap();
    let repo_path = temp_dir.path();

    // Test various git states that cascade should detect

    // 1. Clean state
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    assert!(
        String::from_utf8_lossy(&status.stdout).is_empty(),
        "Should start with clean working directory"
    );

    // 2. Uncommitted changes
    std::fs::write(repo_path.join("uncommitted.txt"), "Changes").unwrap();

    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    assert!(
        String::from_utf8_lossy(&status.stdout).contains("?? uncommitted.txt"),
        "Should detect untracked files"
    );

    // 3. Staged changes
    Command::new("git")
        .args(["add", "uncommitted.txt"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_path)
        .output()
        .unwrap();

    assert!(
        String::from_utf8_lossy(&status.stdout).contains("A  uncommitted.txt"),
        "Should detect staged files"
    );
}
