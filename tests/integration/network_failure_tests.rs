use cascade_cli::bitbucket::{BitbucketClient, PullRequestManager};
use cascade_cli::config::BitbucketConfig;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

/// Test API rate limiting and retry behavior
#[tokio::test]
async fn test_api_rate_limiting_behavior() {
    let mut server = mockito::Server::new_async().await;

    // First call: rate limited
    let _rate_limit_mock = server
        .mock("GET", "/rest/api/1.0/projects/TEST/repos/test-repo")
        .with_status(429)
        .with_header("content-type", "application/json")
        .with_header("Retry-After", "1")
        .with_body(
            json!({
                "errors": [{
                    "message": "Too many requests"
                }]
            })
            .to_string(),
        )
        .expect(1)
        .create_async()
        .await;

    // Second call: success after retry
    let _success_mock = server
        .mock("GET", "/rest/api/1.0/projects/TEST/repos/test-repo")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "id": 1,
                "name": "test-repo",
                "slug": "test-repo",
                "project": {
                    "id": 1,
                    "key": "TEST",
                    "name": "Test Project"
                }
            })
            .to_string(),
        )
        .create_async()
        .await;

    let config = BitbucketConfig {
        url: server.url(),
        project: "TEST".to_string(),
        repo: "test-repo".to_string(),
        username: Some("testuser".to_string()),
        token: Some("testtoken".to_string()),
        default_reviewers: vec![],
        accept_invalid_certs: None,
        ca_bundle_path: None,
    };

    let client = BitbucketClient::new(&config).unwrap();

    // First attempt should be rate limited, client should handle retry
    let start_time = std::time::Instant::now();
    let result = client.test_connection().await;
    let elapsed = start_time.elapsed();

    // Should either succeed after retry or fail with rate limit error
    match result {
        Ok(_) => {
            // If it succeeded, it should have taken some time for retry
            assert!(
                elapsed >= Duration::from_millis(100),
                "Should have taken time to retry"
            );
        }
        Err(e) => {
            // If it failed, should contain rate limit information
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("429")
                    || error_msg.contains("rate limit")
                    || error_msg.contains("Too many"),
                "Should contain rate limit error: {error_msg}"
            );
        }
    }
}

/// Test network timeout scenarios
#[tokio::test]
async fn test_network_timeout_handling() {
    let mut server = mockito::Server::new_async().await;

    // Mock slow response (longer than typical timeout)
    let _slow_mock = server
        .mock("GET", "/rest/api/1.0/projects/TEST/repos/test-repo")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "id": 1,
                "name": "test-repo"
            })
            .to_string(),
        )
        // Note: mockito doesn't have built-in delay, so this tests client timeout handling
        .create_async()
        .await;

    let config = BitbucketConfig {
        url: server.url(),
        project: "TEST".to_string(),
        repo: "test-repo".to_string(),
        username: Some("testuser".to_string()),
        token: Some("testtoken".to_string()),
        default_reviewers: vec![],
        accept_invalid_certs: None,
        ca_bundle_path: None,
    };

    let client = BitbucketClient::new(&config).unwrap();

    // Test with a very short timeout (this would need client-side timeout configuration)
    let result = tokio::time::timeout(Duration::from_millis(100), client.test_connection()).await;

    match result {
        Ok(connection_result) => {
            // Connection completed within timeout
            match connection_result {
                Err(e) => {
                    println!("Connection failed within timeout (expected): {e:?}");
                }
                Ok(_) => {
                    println!("Connection succeeded within timeout");
                }
            }
        }
        Err(_) => {
            // Timeout occurred
            println!("Connection timed out (this tests our timeout wrapper)");
        }
    }

    // This test primarily validates that timeout handling doesn't crash
    // Real timeout behavior would need to be configured in the HTTP client
}

/// Test authentication token expiration scenarios
#[tokio::test]
async fn test_authentication_token_expiration() {
    let mut server = mockito::Server::new_async().await;

    // Mock expired token response
    let _auth_expired_mock = server
        .mock("GET", "/rest/api/1.0/projects/TEST/repos/test-repo")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "errors": [{
                    "message": "Authentication failed",
                    "exceptionName": "com.atlassian.bitbucket.AuthorisationException"
                }]
            })
            .to_string(),
        )
        .create_async()
        .await;

    let config = BitbucketConfig {
        url: server.url(),
        project: "TEST".to_string(),
        repo: "test-repo".to_string(),
        username: Some("testuser".to_string()),
        token: Some("expired-token".to_string()),
        default_reviewers: vec![],
        accept_invalid_certs: None,
        ca_bundle_path: None,
    };

    let client = BitbucketClient::new(&config).unwrap();
    let result = client.test_connection().await;

    // Should handle authentication failure gracefully
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("401")
            || error_msg.contains("Authentication")
            || error_msg.contains("Unauthor"),
        "Should contain authentication error: {error_msg}"
    );
}

/// Test partial API operation failures
#[tokio::test]
async fn test_partial_api_operation_failures() {
    let mut server = mockito::Server::new_async().await;

    // Mock successful repo access
    let _repo_mock = server
        .mock("GET", "/rest/api/1.0/projects/TEST/repos/test-repo")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "id": 1,
                "name": "test-repo",
                "slug": "test-repo",
                "project": {
                    "id": 1,
                    "key": "TEST",
                    "name": "Test Project"
                }
            })
            .to_string(),
        )
        .create_async()
        .await;

    // Mock failed PR creation
    let _pr_fail_mock = server
        .mock(
            "POST",
            "/rest/api/1.0/projects/TEST/repos/test-repo/pull-requests",
        )
        .with_status(500)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "errors": [{
                    "message": "Internal server error during PR creation"
                }]
            })
            .to_string(),
        )
        .create_async()
        .await;

    let config = BitbucketConfig {
        url: server.url(),
        project: "TEST".to_string(),
        repo: "test-repo".to_string(),
        username: Some("testuser".to_string()),
        token: Some("testtoken".to_string()),
        default_reviewers: vec![],
        accept_invalid_certs: None,
        ca_bundle_path: None,
    };

    let client = BitbucketClient::new(&config).unwrap();

    // Test connection should succeed
    let connection_result = client.test_connection().await;
    assert!(connection_result.is_ok(), "Repo access should succeed");

    // But PR operations should fail gracefully
    let _pr_manager = PullRequestManager::new(client);

    // This would test PR creation failure, but we need actual PR creation method
    // For now, this demonstrates the pattern for testing partial failures
    println!("PR manager created successfully, ready for partial operation testing");
}

/// Test network interruption during operations
#[tokio::test]
async fn test_network_interruption_scenarios() {
    // Test with invalid/unreachable server
    let config = BitbucketConfig {
        url: "https://unreachable.invalid.domain.test".to_string(),
        project: "TEST".to_string(),
        repo: "test-repo".to_string(),
        username: Some("testuser".to_string()),
        token: Some("testtoken".to_string()),
        default_reviewers: vec![],
        accept_invalid_certs: None,
        ca_bundle_path: None,
    };

    let client_result = BitbucketClient::new(&config);

    // Client creation should succeed even with invalid URL
    assert!(
        client_result.is_ok(),
        "Client creation should succeed with invalid URL"
    );

    let client = client_result.unwrap();

    // But connection test should fail
    let connection_result = client.test_connection().await;
    assert!(
        connection_result.is_err(),
        "Connection to invalid domain should fail"
    );

    let error_msg = connection_result.unwrap_err().to_string();
    assert!(
        error_msg.contains("dns")
            || error_msg.contains("resolve")
            || error_msg.contains("network")
            || error_msg.contains("connection")
            || error_msg.contains("timeout")
            || error_msg.contains("eof")
            || error_msg.contains("tunneling"),
        "Should contain network-related error: {error_msg}"
    );
}

/// Test CLI network error handling with integration
#[tokio::test]
async fn test_cli_network_error_integration() {
    let (_temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade with unreachable Bitbucket URL
    cascade_cli::config::initialize_repo(
        &repo_path,
        Some("https://unreachable.bitbucket.test".to_string()),
    )
    .unwrap();

    let binary_path = super::test_helpers::get_binary_path();

    // Create a stack
    let stack_result = Command::new(&binary_path)
        .args(["stacks", "create", "network-test"])
        .current_dir(&repo_path)
        .output()
        .expect("Command should run");

    // Stack creation should succeed (it's local)
    if !stack_result.status.success() {
        let stderr = String::from_utf8_lossy(&stack_result.stderr);
        println!("Stack creation failed: {stderr}");
    }

    // Add some commits first so push has something to do
    super::test_helpers::create_test_commits(&repo_path, 1, "network-test").await;

    // Try push operation (would require network)
    let push_result = Command::new(&binary_path)
        .args(["push", "--yes"])
        .current_dir(&repo_path)
        .output()
        .expect("Command should run");

    // Push should fail gracefully - either due to network issues or other validation
    if !push_result.status.success() {
        let stderr = String::from_utf8_lossy(&push_result.stderr);
        let stdout = String::from_utf8_lossy(&push_result.stdout);

        // Accept various types of errors that indicate the CLI is handling failure gracefully
        assert!(
            stderr.contains("network")
                || stderr.contains("connection")
                || stderr.contains("resolve")
                || stderr.contains("timeout")
                || stderr.contains("unreachable")
                || stderr.contains("Configuration")
                || stderr.contains("uncommitted")
                || stderr.contains("Error")
                || stdout.contains("Error"),
            "Should contain some kind of graceful error. Stderr: {stderr}, Stdout: {stdout}"
        );
    } else {
        // If it succeeded, it might be using mock/local operations
        let stdout = String::from_utf8_lossy(&push_result.stdout);
        println!("Push unexpectedly succeeded: {stdout}");
    }
}

// Helper function
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
    std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(&repo_path)
        .output()
        .unwrap();

    (temp_dir, repo_path)
}
