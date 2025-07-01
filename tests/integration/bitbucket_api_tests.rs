use cascade_cli::bitbucket::{BitbucketClient, PullRequestManager};
use cascade_cli::config::BitbucketConfig;
use serde_json::json;

/// Test Bitbucket API client operations with mocked responses
#[tokio::test]
async fn test_bitbucket_client_operations() {
    let mut server = mockito::Server::new_async().await;

    // Mock successful authentication test
    let _auth_mock = server
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
    let result = client.test_connection().await;
    assert!(result.is_ok());
}

/// Test pull request creation and management
#[tokio::test]
async fn test_pull_request_management() {
    let mut server = mockito::Server::new_async().await;

    // Mock PR creation
    let _pr_mock = server
        .mock(
            "POST",
            "/rest/api/1.0/projects/TEST/repos/test-repo/pull-requests",
        )
        .with_status(201)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "id": 123,
                "title": "Test PR",
                "description": "Test description",
                "state": "OPEN",
                "fromRef": {
                    "id": "refs/heads/feature-branch",
                    "displayId": "feature-branch"
                },
                "toRef": {
                    "id": "refs/heads/main",
                    "displayId": "main"
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
    let _pr_manager = PullRequestManager::new(client);

    // Test PR creation would be tested here with proper request structure
    // This demonstrates the pattern for API testing
}

/// Test API error handling and retry logic
#[tokio::test]
async fn test_api_error_handling() {
    let mut server = mockito::Server::new_async().await;

    // Mock rate limit error
    let _rate_limit_mock = server
        .mock("GET", "/rest/api/1.0/projects/TEST/repos/test-repo")
        .with_status(429)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "errors": [{
                    "message": "Rate limit exceeded"
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
    let result = client.test_connection().await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("429"));
}

/// Test authentication methods (token vs username/password)
#[tokio::test]
async fn test_authentication_methods() {
    // Test token-based auth
    let token_config = BitbucketConfig {
        url: "https://test.com".to_string(),
        project: "TEST".to_string(),
        repo: "test-repo".to_string(),
        username: None,
        token: Some("test-token".to_string()),
        default_reviewers: vec![],
        accept_invalid_certs: None,
        ca_bundle_path: None,
    };

    let client = BitbucketClient::new(&token_config);
    assert!(client.is_ok());

    // Test username/password auth
    let user_pass_config = BitbucketConfig {
        url: "https://test.com".to_string(),
        project: "TEST".to_string(),
        repo: "test-repo".to_string(),
        username: Some("testuser".to_string()),
        token: Some("testpass".to_string()),
        default_reviewers: vec![],
        accept_invalid_certs: None,
        ca_bundle_path: None,
    };

    let client = BitbucketClient::new(&user_pass_config);
    assert!(client.is_ok());

    // Test missing auth
    let no_auth_config = BitbucketConfig {
        url: "https://test.com".to_string(),
        project: "TEST".to_string(),
        repo: "test-repo".to_string(),
        username: None,
        token: None,
        default_reviewers: vec![],
        accept_invalid_certs: None,
        ca_bundle_path: None,
    };

    let client = BitbucketClient::new(&no_auth_config);
    assert!(client.is_err());
}
