/// Integration tests for rebase safety to prevent branch corruption
/// These tests ensure that original feature branches remain unchanged during rebase operations
/// and that only the new version branches (v2, v3, etc.) receive the rebased commits.

use cascade_cli::errors::Result;
use cascade_cli::git::GitRepository;
use cascade_cli::stack::{RebaseManager, RebaseOptions, RebaseResult, RebaseStrategy, Stack, StackEntry, StackManager};
use std::process::Command;
use tempfile::TempDir;
use uuid::Uuid;

/// Helper to create a test repository with initial setup
fn create_test_repo_with_stack() -> Result<(TempDir, GitRepository, StackManager, Stack)> {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();
    
    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    
    // Create initial commit on main
    std::fs::write(repo_path.join("README.md"), "# Test Repo").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    
    // Rename master to main
    Command::new("git")
        .args(["branch", "-m", "master", "main"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    
    let git_repo = GitRepository::open(repo_path)?;
    let stack_manager = StackManager::new(repo_path)?;
    
    // Create a stack with entries
    let mut stack = Stack::new("test-stack", "main");
    
    // Create feature branch 1
    git_repo.create_branch("feature-1", Some("main"))?;
    git_repo.checkout_branch("feature-1")?;
    std::fs::write(repo_path.join("feature1.txt"), "Feature 1 content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add feature 1"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    
    let commit1 = git_repo.get_head_commit_hash()?;
    stack.push_entry(StackEntry::new("feature-1", &commit1));
    
    // Create feature branch 2 based on feature-1
    git_repo.create_branch("feature-2", Some("feature-1"))?;
    git_repo.checkout_branch("feature-2")?;
    std::fs::write(repo_path.join("feature2.txt"), "Feature 2 content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add feature 2"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    
    let commit2 = git_repo.get_head_commit_hash()?;
    stack.push_entry(StackEntry::new("feature-2", &commit2));
    
    stack_manager.save_stack(&stack)?;
    
    Ok((temp_dir, git_repo, stack_manager, stack))
}

/// Helper to get all commits on a branch
fn get_branch_commits(repo_path: &std::path::Path, branch: &str) -> Vec<String> {
    let output = Command::new("git")
        .args(["log", "--format=%H %s", "--no-merges", branch])
        .current_dir(repo_path)
        .output()
        .unwrap();
    
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| s.to_string())
        .collect()
}

/// Helper to check if a branch exists
fn branch_exists(repo_path: &std::path::Path, branch: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", branch])
        .current_dir(repo_path)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[test]
fn test_rebase_preserves_original_branches() {
    let (temp_dir, git_repo, stack_manager, stack) = create_test_repo_with_stack().unwrap();
    let repo_path = temp_dir.path();
    
    // Get original commits on feature branches
    let original_feature1_commits = get_branch_commits(repo_path, "feature-1");
    let original_feature2_commits = get_branch_commits(repo_path, "feature-2");
    
    // Perform rebase with branch versioning
    let options = RebaseOptions {
        strategy: RebaseStrategy::BranchVersioning,
        target_base: Some("main".to_string()),
        force_push: false,
        auto_resolve: false,
        preserve_dates: false,
        skip_pull: Some(true),
    };
    
    let mut rebase_manager = RebaseManager::new(git_repo, stack_manager, options);
    let result = rebase_manager.rebase_stack(&stack).unwrap();
    
    assert!(result.success, "Rebase should succeed");
    
    // Verify original branches are completely unchanged
    let after_feature1_commits = get_branch_commits(repo_path, "feature-1");
    let after_feature2_commits = get_branch_commits(repo_path, "feature-2");
    
    assert_eq!(
        original_feature1_commits, after_feature1_commits,
        "Original feature-1 branch should remain unchanged"
    );
    assert_eq!(
        original_feature2_commits, after_feature2_commits,
        "Original feature-2 branch should remain unchanged"
    );
    
    // Verify new version branches were created
    assert!(
        branch_exists(repo_path, "feature-1-v2"),
        "Version 2 of feature-1 should be created"
    );
    assert!(
        branch_exists(repo_path, "feature-2-v2"),
        "Version 2 of feature-2 should be created"
    );
    
    // Verify new branches have the rebased commits
    let v2_feature1_commits = get_branch_commits(repo_path, "feature-1-v2");
    let v2_feature2_commits = get_branch_commits(repo_path, "feature-2-v2");
    
    // New branches should have commits but be different from originals
    assert!(!v2_feature1_commits.is_empty(), "v2 branch should have commits");
    assert!(!v2_feature2_commits.is_empty(), "v2 branch should have commits");
}

#[test]
fn test_rebase_with_conflict_preserves_original() {
    let (temp_dir, git_repo, stack_manager, mut stack) = create_test_repo_with_stack().unwrap();
    let repo_path = temp_dir.path();
    
    // Add a conflicting change on main
    git_repo.checkout_branch("main").unwrap();
    std::fs::write(repo_path.join("feature1.txt"), "Conflicting content on main").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Conflicting change on main"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    
    // Get original commits
    let original_feature1_commits = get_branch_commits(repo_path, "feature-1");
    
    // Attempt rebase (should fail due to conflict)
    let options = RebaseOptions {
        strategy: RebaseStrategy::BranchVersioning,
        target_base: Some("main".to_string()),
        force_push: false,
        auto_resolve: false,
        preserve_dates: false,
        skip_pull: Some(true),
    };
    
    let mut rebase_manager = RebaseManager::new(git_repo, stack_manager, options);
    let result = rebase_manager.rebase_stack(&stack).unwrap();
    
    // Even with conflicts, original branch should be unchanged
    let after_feature1_commits = get_branch_commits(repo_path, "feature-1");
    assert_eq!(
        original_feature1_commits, after_feature1_commits,
        "Original branch should remain unchanged even when rebase has conflicts"
    );
}

#[test]
fn test_multiple_rebases_create_incremental_versions() {
    let (temp_dir, git_repo, stack_manager, stack) = create_test_repo_with_stack().unwrap();
    let repo_path = temp_dir.path();
    
    // Get original state
    let original_feature1_commits = get_branch_commits(repo_path, "feature-1");
    
    // First rebase
    let options = RebaseOptions {
        strategy: RebaseStrategy::BranchVersioning,
        target_base: Some("main".to_string()),
        force_push: false,
        auto_resolve: false,
        preserve_dates: false,
        skip_pull: Some(true),
    };
    
    let mut rebase_manager = RebaseManager::new(git_repo.clone(), stack_manager.clone(), options.clone());
    let result1 = rebase_manager.rebase_stack(&stack).unwrap();
    assert!(result1.success);
    
    // Second rebase
    let mut rebase_manager2 = RebaseManager::new(git_repo.clone(), stack_manager.clone(), options.clone());
    let result2 = rebase_manager2.rebase_stack(&stack).unwrap();
    assert!(result2.success);
    
    // Third rebase
    let mut rebase_manager3 = RebaseManager::new(git_repo, stack_manager, options);
    let result3 = rebase_manager3.rebase_stack(&stack).unwrap();
    assert!(result3.success);
    
    // Verify original is still unchanged
    let final_feature1_commits = get_branch_commits(repo_path, "feature-1");
    assert_eq!(
        original_feature1_commits, final_feature1_commits,
        "Original branch should remain unchanged after multiple rebases"
    );
    
    // Verify all version branches exist
    assert!(branch_exists(repo_path, "feature-1-v2"), "v2 should exist");
    assert!(branch_exists(repo_path, "feature-1-v3"), "v3 should exist");
    assert!(branch_exists(repo_path, "feature-1-v4"), "v4 should exist");
}

#[test]
fn test_cherry_pick_does_not_modify_source_branch() {
    let (temp_dir, git_repo, _, _) = create_test_repo_with_stack().unwrap();
    let repo_path = temp_dir.path();
    
    // Get commit from feature-1
    git_repo.checkout_branch("feature-1").unwrap();
    let commit_to_pick = git_repo.get_head_commit_hash().unwrap();
    let original_commits = get_branch_commits(repo_path, "feature-1");
    
    // Create a new branch and cherry-pick
    git_repo.create_branch("test-target", Some("main")).unwrap();
    git_repo.checkout_branch("test-target").unwrap();
    
    // Cherry-pick the commit
    let new_commit = git_repo.cherry_pick(&commit_to_pick).unwrap();
    
    // Verify source branch is unchanged
    let after_commits = get_branch_commits(repo_path, "feature-1");
    assert_eq!(
        original_commits, after_commits,
        "Source branch should not be modified by cherry-pick"
    );
    
    // Verify new commit exists on target branch
    let target_commits = get_branch_commits(repo_path, "test-target");
    assert!(
        target_commits.iter().any(|c| c.contains("Add feature 1")),
        "Cherry-picked commit should exist on target branch"
    );
}

#[test]
fn test_rebase_rollback_on_failure() {
    let (temp_dir, git_repo, stack_manager, mut stack) = create_test_repo_with_stack().unwrap();
    let repo_path = temp_dir.path();
    
    // Corrupt the second commit hash to force a failure
    stack.entries[1].commit_hash = "invalid_commit_hash".to_string();
    
    // Get original state
    let original_branches: Vec<_> = Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(&repo_path)
        .output()
        .unwrap()
        .stdout
        .lines()
        .map(|l| String::from_utf8(l.unwrap().to_vec()).unwrap())
        .collect();
    
    // Attempt rebase (should fail)
    let options = RebaseOptions {
        strategy: RebaseStrategy::BranchVersioning,
        target_base: Some("main".to_string()),
        force_push: false,
        auto_resolve: false,
        preserve_dates: false,
        skip_pull: Some(true),
    };
    
    let mut rebase_manager = RebaseManager::new(git_repo, stack_manager, options);
    let result = rebase_manager.rebase_stack(&stack);
    
    // Should fail but not panic
    assert!(result.is_err() || !result.unwrap().success);
    
    // No new branches should be created when rebase fails
    let after_branches: Vec<_> = Command::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(&repo_path)
        .output()
        .unwrap()
        .stdout
        .lines()
        .map(|l| String::from_utf8(l.unwrap().to_vec()).unwrap())
        .collect();
    
    // Should not have created any v2 branches
    assert!(
        !after_branches.iter().any(|b| b.contains("-v2")),
        "No version branches should be created when rebase fails"
    );
}

#[test]
fn test_branch_versioning_pattern() {
    let (temp_dir, git_repo, stack_manager, stack) = create_test_repo_with_stack().unwrap();
    let repo_path = temp_dir.path();
    
    // Test various branch naming patterns
    let test_branches = vec![
        ("feature/FOO-123", "feature/FOO-123-v2"),
        ("bugfix/test", "bugfix/test-v2"),
        ("my-branch-v1", "my-branch-v1-v2"), // Already has version
        ("branch-v10", "branch-v10-v2"), // High version number
    ];
    
    for (original, expected_v2) in test_branches {
        // Create branch with commit
        git_repo.create_branch(original, Some("main")).unwrap();
        git_repo.checkout_branch(original).unwrap();
        
        let file_name = format!("{}.txt", original.replace('/', "_"));
        std::fs::write(repo_path.join(&file_name), format!("Content for {}", original)).unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", &format!("Commit for {}", original)])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        
        let commit = git_repo.get_head_commit_hash().unwrap();
        
        // Create single-entry stack
        let mut test_stack = Stack::new(&format!("stack-{}", original), "main");
        test_stack.push_entry(StackEntry::new(original, &commit));
        
        // Rebase
        let options = RebaseOptions {
            strategy: RebaseStrategy::BranchVersioning,
            target_base: Some("main".to_string()),
            force_push: false,
            auto_resolve: false,
            preserve_dates: false,
            skip_pull: Some(true),
        };
        
        let mut rebase_manager = RebaseManager::new(git_repo.clone(), stack_manager.clone(), options);
        let result = rebase_manager.rebase_stack(&test_stack).unwrap();
        
        assert!(result.success);
        assert!(
            branch_exists(repo_path, expected_v2),
            "Expected version branch {} to be created",
            expected_v2
        );
    }
}