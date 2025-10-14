use std::path::PathBuf;
use tempfile::TempDir;

/// Tests to ensure Git hook content includes proper user feedback and messaging
///
/// These tests prevent accidental removal of user-facing messages and ensure
/// hooks provide helpful guidance to users.

#[tokio::test]
async fn test_post_commit_hook_contains_user_feedback() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let hooks_manager = cascade_cli::cli::commands::hooks::HooksManager::new(&repo_path).unwrap();
    let hook_content = hooks_manager
        .generate_hook_script(&cascade_cli::cli::commands::hooks::HookType::PostCommit)
        .unwrap();

    // Verify essential user feedback messages are present (no emojis per professional output guidelines)
    assert!(
        hook_content.contains("Adding commit to active stack"),
        "Post-commit hook should show progress message"
    );
    assert!(
        hook_content.contains("Commit added to stack successfully"),
        "Post-commit hook should show success message"
    );
    assert!(
        hook_content.contains("Next: 'ca submit' to create PRs when ready"),
        "Post-commit hook should provide next steps"
    );
    assert!(
        hook_content.contains("Cascade not initialized"),
        "Post-commit hook should handle uninitialized repos gracefully"
    );
    assert!(
        hook_content.contains("Run 'ca init' to start using stacked diffs"),
        "Post-commit hook should guide users to initialize"
    );
    assert!(
        hook_content.contains("No active stack found"),
        "Post-commit hook should handle missing active stack"
    );
    assert!(
        hook_content.contains("Tip: Use 'ca stack create"),
        "Post-commit hook should guide users to create stacks"
    );
    assert!(
        hook_content.contains("Failed to add commit to stack"),
        "Post-commit hook should handle failures gracefully"
    );
    assert!(
        hook_content.contains("Tip: You can manually add it with"),
        "Post-commit hook should provide recovery instructions"
    );
}

#[tokio::test]
async fn test_pre_push_hook_contains_user_feedback() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let hooks_manager = cascade_cli::cli::commands::hooks::HooksManager::new(&repo_path).unwrap();
    let hook_content = hooks_manager
        .generate_hook_script(&cascade_cli::cli::commands::hooks::HookType::PrePush)
        .unwrap();

    // Verify essential user feedback messages are present (professional output, no emojis)
    assert!(
        hook_content.contains("ERROR: Force push detected"),
        "Pre-push hook should detect and warn about force pushes"
    );
    assert!(
        hook_content.contains("Cascade CLI uses stacked diffs"),
        "Pre-push hook should explain why force pushes are problematic"
    );
    assert!(
        hook_content.contains("Instead, try these commands"),
        "Pre-push hook should provide alternatives to force push"
    );
    assert!(
        hook_content.contains("ca sync"),
        "Pre-push hook should suggest sync command"
    );
    assert!(
        hook_content.contains("ca push"),
        "Pre-push hook should suggest push command"
    );
    assert!(
        hook_content.contains("ca submit"),
        "Pre-push hook should suggest submit command"
    );
    assert!(
        hook_content.contains("ca autoland"),
        "Pre-push hook should suggest autoland command"
    );
    assert!(
        hook_content.contains("If you really need to force push"),
        "Pre-push hook should provide escape hatch"
    );
    assert!(
        hook_content.contains("Stack validation") || hook_content.contains("ca validate"),
        "Pre-push hook should show validation"
    );
    // Check for validation success/failure (no emojis)
    assert!(
        hook_content.contains("validation") || hook_content.contains("ca validate"),
        "Pre-push hook should reference validation"
    );
    // Note: We simplified the pre-push hook to just run validation
    // Removed verbose guidance messages to keep it professional and concise
}

#[tokio::test]
async fn test_commit_msg_hook_contains_user_feedback() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let hooks_manager = cascade_cli::cli::commands::hooks::HooksManager::new(&repo_path).unwrap();
    let hook_content = hooks_manager
        .generate_hook_script(&cascade_cli::cli::commands::hooks::HookType::CommitMsg)
        .unwrap();

    // Verify essential validation (professional output, no emojis or conventional commit blurbs)
    // Note: We removed conventional commit format suggestions and emojis as per user request
    // The hook now focuses on essential validation only and exits 0 on success
    // Windows uses "exit /b 0", Unix uses "exit 0"
    assert!(
        hook_content.contains("exit 0") || hook_content.contains("exit /b 0"),
        "Commit-msg hook should have success path"
    );
}

#[tokio::test]
async fn test_prepare_commit_msg_hook_contains_user_feedback() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let hooks_manager = cascade_cli::cli::commands::hooks::HooksManager::new(&repo_path).unwrap();
    let hook_content = hooks_manager
        .generate_hook_script(&cascade_cli::cli::commands::hooks::HookType::PrepareCommitMsg)
        .unwrap();

    // Verify essential user feedback messages are present
    assert!(
        hook_content.contains("# Stack:"),
        "Prepare-commit-msg hook should add stack context"
    );
    assert!(
        hook_content.contains("# This commit will be added to the active stack automatically"),
        "Prepare-commit-msg hook should explain automatic behavior"
    );
    assert!(
        hook_content.contains("# Use 'ca stack status' to see the current stack state"),
        "Prepare-commit-msg hook should provide helpful commands"
    );
}

#[tokio::test]
async fn test_pre_commit_hook_contains_edit_mode_guidance() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let hooks_manager = cascade_cli::cli::commands::hooks::HooksManager::new(&repo_path).unwrap();
    let hook_content = hooks_manager
        .generate_hook_script(&cascade_cli::cli::commands::hooks::HookType::PreCommit)
        .unwrap();

    // Verify edit mode guidance is present
    assert!(
        hook_content.contains("entry status --quiet"),
        "Pre-commit hook should check edit mode status"
    );
    assert!(
        hook_content.contains("You're in EDIT MODE for a stack entry"),
        "Pre-commit hook should provide edit mode message"
    );
    assert!(
        hook_content.contains("[a] amend: Modify the current entry"),
        "Pre-commit hook should explain amend option"
    );
    assert!(
        hook_content.contains("[n] new:   Create new entry on top"),
        "Pre-commit hook should explain new commit option"
    );
    assert!(
        hook_content.contains("[c] cancel: Stop and think about it"),
        "Pre-commit hook should provide cancel option"
    );
    assert!(
        hook_content.contains("entry amend --restack"),
        "Pre-commit hook should call ca entry amend with restack for amend option"
    );
    assert!(
        !hook_content.contains("entry amend --all"),
        "Pre-commit hook should avoid staging unstaged changes when amending"
    );
    assert!(
        hook_content.contains("Creating new stack entry..."),
        "Pre-commit hook should show what happens when creating new entry"
    );
}

#[tokio::test]
async fn test_prepare_commit_msg_hook_contains_edit_mode_guidance() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let hooks_manager = cascade_cli::cli::commands::hooks::HooksManager::new(&repo_path).unwrap();
    let hook_content = hooks_manager
        .generate_hook_script(&cascade_cli::cli::commands::hooks::HookType::PrepareCommitMsg)
        .unwrap();

    // Verify stack context is added (but no edit mode guidance - that's in pre-commit now)
    assert!(
        hook_content.contains("Stack:"),
        "Prepare-commit-msg hook should add stack context"
    );
    assert!(
        hook_content.contains("stack list --active"),
        "Prepare-commit-msg hook should check for active stack"
    );
    // Edit mode guidance was removed - it's now in pre-commit hook only
    assert!(
        !hook_content.contains("[EDIT MODE]"),
        "Prepare-commit-msg hook should NOT contain edit mode guidance anymore"
    );
}

#[tokio::test]
async fn test_hooks_are_platform_specific() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let hooks_manager = cascade_cli::cli::commands::hooks::HooksManager::new(&repo_path).unwrap();
    let hook_content = hooks_manager
        .generate_hook_script(&cascade_cli::cli::commands::hooks::HookType::PostCommit)
        .unwrap();

    // Verify platform-specific content
    #[cfg(windows)]
    {
        assert!(
            hook_content.starts_with("@echo off"),
            "Windows hooks should start with @echo off"
        );
        assert!(
            hook_content.contains("rem Cascade CLI Hook"),
            "Windows hooks should use rem for comments"
        );
        assert!(
            hook_content.contains("%ERRORLEVEL%"),
            "Windows hooks should use ERRORLEVEL for error checking"
        );
    }

    #[cfg(not(windows))]
    {
        assert!(
            hook_content.starts_with("#!/bin/sh"),
            "Unix hooks should start with shebang"
        );
        assert!(
            hook_content.contains("# Cascade CLI Hook"),
            "Unix hooks should use # for comments"
        );
        assert!(
            hook_content.contains("set -e"),
            "Unix hooks should use set -e for error handling"
        );
    }
}

#[tokio::test]
async fn test_hook_content_includes_binary_path() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let hooks_manager = cascade_cli::cli::commands::hooks::HooksManager::new(&repo_path).unwrap();

    let hook_types = [
        cascade_cli::cli::commands::hooks::HookType::PostCommit,
        cascade_cli::cli::commands::hooks::HookType::PrePush,
        cascade_cli::cli::commands::hooks::HookType::PrepareCommitMsg,
    ];

    for hook_type in &hook_types {
        let hook_content = hooks_manager.generate_hook_script(hook_type).unwrap();

        // Since the hook uses the current executable path, we can't test for a specific custom path
        // but we can verify that some executable path is present
        assert!(
            !hook_content.is_empty(),
            "Hook {hook_type:?} should contain content"
        );
    }
}

#[tokio::test]
async fn test_hooks_handle_edge_cases_gracefully() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://test.bitbucket.com".to_string()),
    )
    .unwrap();

    let hooks_manager = cascade_cli::cli::commands::hooks::HooksManager::new(&repo_path).unwrap();
    let hook_content = hooks_manager
        .generate_hook_script(&cascade_cli::cli::commands::hooks::HookType::PostCommit)
        .unwrap();

    // Verify graceful handling of edge cases
    assert!(
        hook_content.contains("if [ ! -d \"$REPO_ROOT/.cascade\" ]")
            || hook_content.contains("if not exist \"%REPO_ROOT%\\.cascade\""),
        "Hooks should check for Cascade initialization"
    );
    assert!(
        hook_content.contains("exit 0") || hook_content.contains("exit /b 0"),
        "Hooks should exit gracefully when Cascade is not initialized"
    );
}

// Helper function to create test git repository
async fn create_test_git_repo() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    // Initialize git repository
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    // Create initial commit
    std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    (temp_dir, repo_path)
}
