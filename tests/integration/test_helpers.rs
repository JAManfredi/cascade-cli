use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

/// Test helper macros and utilities for reducing boilerplate
///
/// This module provides common test setup patterns and helper functions
/// to improve test reliability and reduce code duplication.
/// Timeout wrapper for CLI operations to prevent hanging tests
pub async fn run_cli_with_timeout(
    binary_path: &Path,
    args: &[&str],
    repo_path: &Path,
    timeout_duration: Duration,
) -> Result<std::process::Output, String> {
    let binary_path = binary_path.to_path_buf();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let repo_path = repo_path.to_path_buf();

    let command_future = tokio::task::spawn_blocking(move || {
        Command::new(&binary_path)
            .args(&args)
            .current_dir(&repo_path)
            .env("RUST_LOG", "info")
            .output()
    });

    match tokio::time::timeout(timeout_duration, command_future).await {
        Ok(task_result) => match task_result {
            Ok(io_result) => match io_result {
                Ok(output) => Ok(output),
                Err(e) => Err(format!("Command execution failed: {e}")),
            },
            Err(e) => Err(format!("Task panicked: {e}")),
        },
        Err(_) => Err(format!("Command timed out after {timeout_duration:?}")),
    }
}

/// Create test git repository with standard setup
pub async fn create_test_git_repo() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    // Initialize git with standard config
    let git_commands = [
        vec!["init"],
        vec!["config", "user.name", "Test User"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "init.defaultBranch", "main"],
    ];

    for cmd_args in &git_commands {
        Command::new("git")
            .args(cmd_args)
            .current_dir(&repo_path)
            .output()
            .expect("Git command should succeed");
    }

    // Create initial commit
    std::fs::write(repo_path.join("README.md"), "# Test Repository").unwrap();
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

    (temp_dir, repo_path)
}

/// Create test git repository with cascade initialization
pub async fn create_test_cascade_repo(bitbucket_url: Option<String>) -> (TempDir, PathBuf) {
    let (temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade
    let url = bitbucket_url.unwrap_or_else(|| "https://test.bitbucket.com".to_string());
    cascade_cli::config::initialize_repo(&repo_path, Some(url))
        .expect("Cascade initialization should succeed");

    (temp_dir, repo_path)
}

/// Create multiple test commits with consistent naming
#[allow(dead_code)]
pub async fn create_test_commits(repo_path: &PathBuf, count: u32, prefix: &str) {
    for i in 1..=count {
        let filename = format!("{prefix}-{i}.txt");
        let content = format!(
            "Content for {prefix} file {i}\nCreated at: {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
        );

        std::fs::write(repo_path.join(&filename), content).unwrap();

        Command::new("git")
            .args(["add", &filename])
            .current_dir(repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["commit", "-m", &format!("{prefix}: Add file {i}")])
            .current_dir(repo_path)
            .output()
            .unwrap();
    }
}

/// Assert CLI command succeeds with helpful error messages
#[allow(dead_code)]
pub fn assert_cli_success(output: &std::process::Output, operation: &str) {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "{operation} failed:\nExit code: {}\nStderr: {stderr}\nStdout: {stdout}",
            output.status.code().unwrap_or(-1)
        );
    }
}

/// Assert CLI command fails with specific error pattern
#[allow(dead_code)]
pub fn assert_cli_error_contains(
    output: &std::process::Output,
    operation: &str,
    expected_error: &str,
) {
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!("{operation} unexpectedly succeeded. Stdout: {stdout}");
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stderr.contains(expected_error) || stdout.contains(expected_error),
        "{operation} failed but didn't contain expected error '{expected_error}'.\nStderr: {stderr}\nStdout: {stdout}"
    );
}

/// Check if CLI command output contains expected content
#[allow(dead_code)]
pub fn assert_output_contains(
    output: &std::process::Output,
    expected_content: &str,
    context: &str,
) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stderr.contains(expected_content) || stdout.contains(expected_content),
        "{context}: Expected to find '{expected_content}' in output.\nStderr: {stderr}\nStdout: {stdout}"
    );
}

/// Get binary path with caching for performance
pub fn get_binary_path() -> PathBuf {
    let current_dir = std::env::current_dir().unwrap();

    // Try release binary first, fall back to debug binary for CI
    let release_binary = current_dir.join("target/release/cc");
    let debug_binary = current_dir.join("target/debug/cc");

    if release_binary.exists() {
        release_binary
    } else {
        debug_binary
    }
}

/// Retry wrapper for flaky operations
#[allow(dead_code)]
pub async fn retry_operation<F, T, E>(
    operation: F,
    max_attempts: u32,
    delay: Duration,
    operation_name: &str,
) -> Result<T, E>
where
    F: Fn() -> Result<T, E>,
    E: std::fmt::Debug,
{
    for attempt in 1..=max_attempts {
        match operation() {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt == max_attempts {
                    eprintln!("{operation_name} failed after {max_attempts} attempts: {e:?}");
                    return Err(e);
                }
                eprintln!("{operation_name} attempt {attempt} failed: {e:?}, retrying...");
                tokio::time::sleep(delay).await;
            }
        }
    }
    unreachable!()
}

/// Parallel test execution helper
pub async fn run_parallel_operations<F, T>(
    operations: Vec<F>,
    operation_name: String,
) -> Vec<Result<T, String>>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    let handles: Vec<_> = operations
        .into_iter()
        .enumerate()
        .map(|(i, op)| {
            let operation_name = operation_name.clone();
            tokio::task::spawn_blocking(move || match op() {
                Ok(result) => Ok(result),
                Err(e) => Err(format!("{operation_name} #{i} failed: {e}")),
            })
        })
        .collect();

    let results = futures::future::join_all(handles).await;

    results
        .into_iter()
        .map(|handle_result| handle_result.unwrap_or_else(|e| Err(format!("Task panicked: {e}"))))
        .collect()
}

/// Test fixture for pre-configured test environment
#[allow(dead_code)]
pub struct TestFixture {
    #[allow(dead_code)]
    pub temp_dir: TempDir,
    pub repo_path: PathBuf,
    pub binary_path: PathBuf,
}

#[allow(dead_code)]
impl TestFixture {
    #[allow(dead_code)]
    pub async fn new() -> Self {
        let (temp_dir, repo_path) = create_test_cascade_repo(None).await;
        let binary_path = get_binary_path();

        Self {
            temp_dir,
            repo_path,
            binary_path,
        }
    }

    #[allow(dead_code)]
    pub async fn new_with_bitbucket_url(url: String) -> Self {
        let (temp_dir, repo_path) = create_test_cascade_repo(Some(url)).await;
        let binary_path = get_binary_path();

        Self {
            temp_dir,
            repo_path,
            binary_path,
        }
    }

    #[allow(dead_code)]
    pub async fn run_cli(&self, args: &[&str]) -> std::process::Output {
        run_cli_with_timeout(
            &self.binary_path,
            args,
            &self.repo_path,
            Duration::from_secs(30),
        )
        .await
        .expect("CLI command should complete within timeout")
    }

    #[allow(dead_code)]
    pub async fn run_cli_expect_success(
        &self,
        args: &[&str],
        operation: &str,
    ) -> std::process::Output {
        let output = self.run_cli(args).await;
        assert_cli_success(&output, operation);
        output
    }

    #[allow(dead_code)]
    pub async fn run_cli_expect_error(
        &self,
        args: &[&str],
        operation: &str,
        expected_error: &str,
    ) -> std::process::Output {
        let output = self.run_cli(args).await;
        assert_cli_error_contains(&output, operation, expected_error);
        output
    }

    #[allow(dead_code)]
    pub async fn create_commits(&self, count: u32, prefix: &str) {
        create_test_commits(&self.repo_path, count, prefix).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fixture_creation() {
        let fixture = TestFixture::new().await;

        // Verify git repo is properly set up
        assert!(fixture.repo_path.join(".git").exists());
        assert!(fixture.repo_path.join("README.md").exists());
        assert!(fixture.repo_path.join(".cascade").exists());

        // Verify binary exists
        assert!(
            fixture.binary_path.exists(),
            "Binary should be built before running tests"
        );
    }

    #[tokio::test]
    async fn test_timeout_wrapper() {
        let fixture = TestFixture::new().await;

        // Test successful command with timeout
        let result = run_cli_with_timeout(
            &fixture.binary_path,
            &["--help"],
            &fixture.repo_path,
            Duration::from_secs(5),
        )
        .await;

        assert!(result.is_ok(), "Help command should succeed");
    }

    #[tokio::test]
    async fn test_parallel_operations() {
        let operations = vec![
            || Ok("result1".to_string()),
            || Ok("result2".to_string()),
            || Err("error".to_string()),
        ];

        let results = run_parallel_operations(operations, "test_op".to_string()).await;

        assert_eq!(results.len(), 3);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        assert!(results[2].is_err());
    }
}
