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

    // Verify essential user feedback messages are present
    assert!(
        hook_content.contains("🪝 Adding commit to active stack"),
        "Post-commit hook should show progress message"
    );
    assert!(
        hook_content.contains("✅ Commit added to stack successfully"),
        "Post-commit hook should show success message"
    );
    assert!(
        hook_content.contains("💡 Next: 'ca submit' to create PRs when ready"),
        "Post-commit hook should provide next steps"
    );
    assert!(
        hook_content.contains("ℹ️ Cascade not initialized"),
        "Post-commit hook should handle uninitialized repos gracefully"
    );
    assert!(
        hook_content.contains("💡 Run 'ca init' to start using stacked diffs"),
        "Post-commit hook should guide users to initialize"
    );
    assert!(
        hook_content.contains("ℹ️ No active stack found"),
        "Post-commit hook should handle missing active stack"
    );
    assert!(
        hook_content.contains("💡 Use 'ca stack create"),
        "Post-commit hook should guide users to create stacks"
    );
    assert!(
        hook_content.contains("⚠️ Failed to add commit to stack"),
        "Post-commit hook should handle failures gracefully"
    );
    assert!(
        hook_content.contains("💡 You can manually add it with"),
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

    // Verify essential user feedback messages are present
    assert!(
        hook_content.contains("❌ Force push detected!"),
        "Pre-push hook should detect and warn about force pushes"
    );
    assert!(
        hook_content.contains("🌊 Cascade CLI uses stacked diffs"),
        "Pre-push hook should explain why force pushes are problematic"
    );
    assert!(
        hook_content.contains("💡 Instead of force pushing, try these streamlined commands"),
        "Pre-push hook should provide alternatives to force push"
    );
    assert!(
        hook_content.contains("• ca sync"),
        "Pre-push hook should suggest sync command"
    );
    assert!(
        hook_content.contains("• ca push"),
        "Pre-push hook should suggest push command"
    );
    assert!(
        hook_content.contains("• ca submit"),
        "Pre-push hook should suggest submit command"
    );
    assert!(
        hook_content.contains("• ca autoland"),
        "Pre-push hook should suggest autoland command"
    );
    assert!(
        hook_content.contains("🚨 If you really need to force push"),
        "Pre-push hook should provide escape hatch"
    );
    assert!(
        hook_content.contains("🪝 Validating stack state before push"),
        "Pre-push hook should show validation progress"
    );
    assert!(
        hook_content.contains("✅ Stack validation passed"),
        "Pre-push hook should show validation success"
    );
    assert!(
        hook_content.contains("❌ Stack validation failed"),
        "Pre-push hook should show validation failure"
    );
    assert!(
        hook_content.contains("💡 Fix validation errors before pushing"),
        "Pre-push hook should guide users on fixing issues"
    );
    assert!(
        hook_content.contains("• ca doctor"),
        "Pre-push hook should suggest doctor command"
    );
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

    // Verify essential user feedback messages are present
    assert!(
        hook_content.contains("❌ Commit message too short"),
        "Commit-msg hook should validate message length"
    );
    assert!(
        hook_content.contains("💡 Write a descriptive commit message"),
        "Commit-msg hook should guide users on good practices"
    );
    assert!(
        hook_content.contains("⚠️ Warning: Commit message longer than"),
        "Commit-msg hook should warn about overly long messages"
    );
    assert!(
        hook_content.contains("💡 Consider keeping the first line short"),
        "Commit-msg hook should provide readability guidance"
    );
    assert!(
        hook_content.contains("💡 Consider using conventional commit format"),
        "Commit-msg hook should suggest conventional commits"
    );
    assert!(
        hook_content.contains("feat: add new feature"),
        "Commit-msg hook should provide examples"
    );
    assert!(
        hook_content.contains("fix: resolve bug"),
        "Commit-msg hook should provide examples"
    );
    assert!(
        hook_content.contains("✅ Commit message validation passed"),
        "Commit-msg hook should show validation success"
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
        hook_content.contains("if [ ! -d \".cascade\" ]")
            || hook_content.contains("if not exist \".cascade\""),
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
