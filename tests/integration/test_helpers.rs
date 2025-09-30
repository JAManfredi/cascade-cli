use git2::{Repository, Signature, Time};
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
        let mut cmd = Command::new(&binary_path);
        cmd.args(&args)
            .current_dir(&repo_path)
            .env("RUST_LOG", "info")
            .env("_CC_COMPLETE", "") // Prevent shell completion activation
            .env("COMPLETE", ""); // Prevent alternative completion activation

        // Windows-specific environment setup
        if cfg!(windows) {
            cmd.env("HOME", std::env::var("USERPROFILE").unwrap_or_default())
                .env("TERM", "xterm"); // Some CLI tools expect TERM to be set
        }

        cmd.output()
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
    let mut git_commands = vec![
        vec!["init"],
        vec!["config", "user.name", "Test User"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "init.defaultBranch", "main"],
        vec!["config", "core.autocrlf", "false"], // Prevent line ending issues
        vec!["config", "core.filemode", "false"], // Prevent file mode issues
    ];

    // Windows-specific git configuration
    if cfg!(windows) {
        git_commands.extend(vec![
            vec!["config", "core.longpaths", "true"], // Support long paths on Windows
            vec!["config", "core.preloadindex", "true"], // Performance optimization
            vec!["config", "core.fscache", "true"],   // File system cache for Windows
        ]);
    }

    for cmd_args in &git_commands {
        let output = Command::new("git")
            .args(cmd_args)
            .current_dir(&repo_path)
            .output()
            .expect("Git command should succeed");

        if !output.status.success() {
            panic!(
                "Git command failed: git {}\nStderr: {}",
                cmd_args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        }
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
#[allow(dead_code)]
pub async fn create_test_cascade_repo(bitbucket_url: Option<String>) -> (TempDir, PathBuf) {
    let (temp_dir, repo_path) = create_test_git_repo().await;

    // Initialize cascade with retry logic for reliability
    let url = bitbucket_url.unwrap_or_else(|| "https://test.bitbucket.com".to_string());

    for attempt in 1..=3 {
        match cascade_cli::config::initialize_repo(&repo_path, Some(url.clone())) {
            Ok(_) => break,
            Err(e) if attempt == 3 => panic!("Cascade initialization failed after 3 attempts: {e}"),
            Err(e) => {
                eprintln!("Cascade initialization attempt {attempt} failed: {e}, retrying...");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }

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
    // Use a more stable approach that doesn't rely on current_dir
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let current_dir = PathBuf::from(manifest_dir);

    // CI environments usually build release binaries
    let release_binary = current_dir
        .join("target/release")
        .join(cascade_cli::utils::platform::executable_name("ca"));
    let debug_binary = current_dir
        .join("target/debug")
        .join(cascade_cli::utils::platform::executable_name("ca"));

    // Try release first, then debug
    if release_binary.exists() && cascade_cli::utils::platform::is_executable(&release_binary) {
        release_binary
    } else if debug_binary.exists() && cascade_cli::utils::platform::is_executable(&debug_binary) {
        debug_binary
    } else {
        panic!(
            "No executable binary found. Tried:\n  - {}\n  - {}\n\nRun 'cargo build --release' first.",
            release_binary.display(),
            debug_binary.display()
        );
    }
}

/// Retry wrapper for flaky operations with exponential backoff
#[allow(dead_code)]
pub async fn retry_operation<F, T, E>(
    mut operation: F,
    max_attempts: u32,
    base_delay: Duration,
    operation_name: &str,
) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
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

                // Exponential backoff with jitter
                let delay = base_delay * 2_u32.pow(attempt - 1);
                let jitter = Duration::from_millis(fastrand::u64(0..100));
                let total_delay = delay + jitter;

                eprintln!(
                    "{operation_name} attempt {attempt} failed: {e:?}. Retrying in {total_delay:?}..."
                );
                tokio::time::sleep(total_delay).await;
            }
        }
    }
    unreachable!()
}

/// Run parallel operations with better error handling and resource management
pub async fn run_parallel_operations<F, T>(
    operations: Vec<F>,
    operation_name: String,
) -> Vec<Result<T, String>>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    // Limit concurrency based on environment and platform
    let max_concurrency = std::env::var("INTEGRATION_TEST_CONCURRENCY")
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "1".to_string() // Windows is more sensitive to concurrency
            } else {
                "2".to_string() // Unix can handle more concurrency
            }
        })
        .parse::<usize>()
        .unwrap_or(1);

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrency));
    let mut handles = Vec::new();

    for (i, operation) in operations.into_iter().enumerate() {
        let semaphore = semaphore.clone();
        let operation_name = operation_name.clone();

        let handle = tokio::spawn(async move {
            let _permit = semaphore
                .acquire()
                .await
                .expect("Semaphore should not be closed");

            // Add jitter to prevent thundering herd
            let jitter_max = if cfg!(windows) {
                150 // Windows needs longer delays
            } else {
                50 // Shorter delays for Unix
            };
            let jitter = Duration::from_millis(fastrand::u64(0..jitter_max));
            tokio::time::sleep(jitter).await;

            let result = tokio::task::spawn_blocking(operation).await;

            match result {
                Ok(inner_result) => inner_result,
                Err(join_error) => Err(format!("{operation_name}[{i}] panicked: {join_error}")),
            }
        });

        handles.push(handle);
    }

    // Collect results with timeout
    let timeout_duration = Duration::from_secs(
        std::env::var("TEST_TIMEOUT")
            .unwrap_or_else(|_| "120".to_string())
            .parse::<u64>()
            .unwrap_or(120),
    );

    let mut results = Vec::new();
    for (i, handle) in handles.into_iter().enumerate() {
        match tokio::time::timeout(timeout_duration, handle).await {
            Ok(Ok(result)) => results.push(result),
            Ok(Err(join_error)) => {
                results.push(Err(format!(
                    "{operation_name}[{i}] join error: {join_error}"
                )));
            }
            Err(_) => {
                results.push(Err(format!(
                    "{operation_name}[{i}] timed out after {timeout_duration:?}"
                )));
            }
        }
    }

    results
}

/// Test fixture with automatic cleanup and standard configuration
pub struct TestFixture {
    #[allow(dead_code)]
    pub temp_dir: TempDir,
    pub repo_path: PathBuf,
    pub binary_path: PathBuf,
}

impl TestFixture {
    /// Create a new test fixture with standard git repository
    pub async fn new() -> Self {
        let (temp_dir, repo_path) = create_test_git_repo().await;
        let binary_path = get_binary_path();

        Self {
            temp_dir,
            repo_path,
            binary_path,
        }
    }

    /// Create a new test fixture with cascade initialization
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

    /// Run CLI command with timeout
    #[allow(dead_code)]
    pub async fn run_cli(&self, args: &[&str]) -> std::process::Output {
        let timeout = Duration::from_secs(
            std::env::var("TEST_TIMEOUT")
                .unwrap_or_else(|_| "60".to_string())
                .parse::<u64>()
                .unwrap_or(60),
        );

        run_cli_with_timeout(&self.binary_path, args, &self.repo_path, timeout)
            .await
            .unwrap_or_else(|e| panic!("CLI command failed: {e}"))
    }

    /// Run CLI command and assert success
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

    /// Run CLI command and assert specific error
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

    /// Create test commits
    #[allow(dead_code)]
    pub async fn create_commits(&self, count: u32, prefix: &str) {
        create_test_commits(&self.repo_path, count, prefix).await;
    }
}

/// Create a test repository with specified number of commits using git2
/// Returns (Repository, Vec<commit_oids>) to avoid lifetime issues
#[allow(dead_code)]
pub fn create_test_repo_with_commits(
    repo_path: &Path,
    commit_count: usize,
) -> Result<(Repository, Vec<String>), String> {
    // Initialize repository
    let repo =
        Repository::init(repo_path).map_err(|e| format!("Failed to initialize repository: {e}"))?;

    // Set up configuration
    let mut config = repo
        .config()
        .map_err(|e| format!("Failed to get repository config: {e}"))?;

    config
        .set_str("user.name", "Test User")
        .map_err(|e| format!("Failed to set user.name: {e}"))?;
    config
        .set_str("user.email", "test@example.com")
        .map_err(|e| format!("Failed to set user.email: {e}"))?;

    let signature = Signature::new("Test User", "test@example.com", &Time::new(1234567890, 0))
        .map_err(|e| format!("Failed to create signature: {e}"))?;

    let mut commit_oids = Vec::new();
    let mut parent_oid: Option<git2::Oid> = None;

    for i in 0..commit_count {
        // Create a test file
        let filename = format!("test-file-{}.txt", i + 1);
        let filepath = repo_path.join(&filename);
        let content = format!("Content for commit {}\nTimestamp: {}", i + 1, i * 1000);

        std::fs::write(&filepath, content)
            .map_err(|e| format!("Failed to write test file: {e}"))?;

        // Add file to index
        let mut index = repo
            .index()
            .map_err(|e| format!("Failed to get repository index: {e}"))?;

        index
            .add_path(std::path::Path::new(&filename))
            .map_err(|e| format!("Failed to add file to index: {e}"))?;

        index
            .write()
            .map_err(|e| format!("Failed to write index: {e}"))?;

        // Create tree from index
        let tree_oid = index
            .write_tree()
            .map_err(|e| format!("Failed to write tree: {e}"))?;

        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| format!("Failed to find tree: {e}"))?;

        // Create commit
        let message = format!("Test commit {}", i + 1);
        let parents: Vec<git2::Commit> = if let Some(parent_oid) = parent_oid {
            vec![repo
                .find_commit(parent_oid)
                .map_err(|e| format!("Failed to find parent commit: {e}"))?]
        } else {
            vec![] // First commit has no parents
        };

        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

        let commit_oid = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                &message,
                &tree,
                &parent_refs,
            )
            .map_err(|e| format!("Failed to create commit: {e}"))?;

        commit_oids.push(commit_oid.to_string());
        parent_oid = Some(commit_oid);
    }

    Ok((repo, commit_oids))
}

/// Simplified version for most common use case
#[allow(dead_code)]
pub async fn run_cc_with_timeout(args: &[&str], timeout_ms: u64) -> std::process::Output {
    let binary_path = get_binary_path();
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    let timeout = Duration::from_millis(timeout_ms);

    match run_cli_with_timeout(&binary_path, args, &current_dir, timeout).await {
        Ok(output) => output,
        Err(e) => panic!("CLI command failed: {e}"),
    }
}

/// Run cascade CLI with timeout in a specific directory
#[allow(dead_code)]
pub async fn run_cc_with_timeout_in_dir(
    args: &[&str],
    timeout_ms: u64,
    repo_path: &Path,
) -> std::process::Output {
    let binary_path = get_binary_path();
    let timeout = Duration::from_millis(timeout_ms);

    match run_cli_with_timeout(&binary_path, args, repo_path, timeout).await {
        Ok(output) => output,
        Err(e) => panic!("CLI command failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fixture_creation() {
        let fixture = TestFixture::new().await;

        // Test that fixture is properly initialized
        assert!(fixture.repo_path.exists());
        assert!(fixture.binary_path.exists());

        // Test git is properly configured
        let output = Command::new("git")
            .args(["config", "user.name"])
            .current_dir(&fixture.repo_path)
            .output()
            .unwrap();

        let username = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(username, "Test User");

        // Test initial commit exists
        let output = Command::new("git")
            .args(["log", "--oneline"])
            .current_dir(&fixture.repo_path)
            .output()
            .unwrap();

        assert!(!output.stdout.is_empty());
    }

    #[tokio::test]
    async fn test_timeout_wrapper() {
        let fixture = TestFixture::new().await;

        // Test successful command
        let result = run_cli_with_timeout(
            &fixture.binary_path,
            &["--help"],
            &fixture.repo_path,
            Duration::from_secs(5),
        )
        .await;

        assert!(result.is_ok(), "Help command should succeed");

        // Test timeout (using a command that should timeout)
        let result = run_cli_with_timeout(
            &fixture.binary_path,
            &["stacks", "list"], // This might hang without proper setup
            &fixture.repo_path,
            Duration::from_millis(10), // Very short timeout
        )
        .await;

        // Should either succeed quickly or timeout
        if let Err(error_msg) = result {
            assert!(error_msg.contains("timed out"));
        }
    }

    #[tokio::test]
    async fn test_parallel_operations() {
        let operations: Vec<Box<dyn FnOnce() -> Result<String, String> + Send>> = (0..3)
            .map(|i| {
                let closure: Box<dyn FnOnce() -> Result<String, String> + Send> =
                    Box::new(move || {
                        std::thread::sleep(Duration::from_millis(10));
                        Ok(format!("result-{i}"))
                    });
                closure
            })
            .collect();

        let results = run_parallel_operations(operations, "test_parallel".to_string()).await;

        assert_eq!(results.len(), 3);
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "Operation {i} should succeed");
            assert_eq!(result.as_ref().unwrap(), &format!("result-{i}"));
        }
    }
}

/// Initialize Cascade in a repository
pub fn run_cascade_init(repo_path: &Path) {
    let output = Command::new("ca")
        .current_dir(repo_path)
        .args(["init", "--bitbucket-url", "https://test.bitbucket.com"])
        .output()
        .expect("Failed to run ca init");
    
    assert!(output.status.success(), "ca init failed: {}", String::from_utf8_lossy(&output.stderr));
}

/// Get current branch name
pub fn git_current_branch(repo_path: &Path) -> String {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["branch", "--show-current"])
        .output()
        .expect("Failed to get current branch");
    
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Get HEAD commit hash
pub fn git_head_hash(repo_path: &Path) -> String {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("Failed to get HEAD hash");
    
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Get commit hash for a specific branch
pub fn git_branch_hash(repo_path: &Path, branch: &str) -> String {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["rev-parse", branch])
        .output()
        .expect("Failed to get branch hash");
    
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Create a file with content
pub fn create_file(repo_path: &Path, filename: &str, content: &str) {
    let file_path = repo_path.join(filename);
    std::fs::write(file_path, content).expect("Failed to create file");
}

/// Git add all changes
pub fn git_add_all(repo_path: &Path) {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["add", "-A"])
        .output()
        .expect("Failed to git add");
    
    assert!(output.status.success(), "git add failed");
}

/// Create a test repo with N commits and return temp dir + path
pub async fn create_temp_repo_with_commits(count: u32) -> (TempDir, PathBuf) {
    let (temp_dir, repo_path) = create_test_git_repo().await;
    
    for i in 1..=count {
        create_file(&repo_path, &format!("file{}.txt", i), &format!("content {}", i));
        git_add_all(&repo_path);
        
        let output = Command::new("git")
            .current_dir(&repo_path)
            .args(["commit", "-m", &format!("Commit {}", i)])
            .output()
            .expect("Failed to commit");
        
        assert!(output.status.success(), "Failed to create commit {}", i);
    }
    
    (temp_dir, repo_path)
}

/// Run a git commit with a message
pub fn git_commit(repo_path: &Path, message: &str) {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["commit", "-m", message])
        .output()
        .expect("Failed to run git commit");
    
    assert!(output.status.success(), "git commit failed: {}", String::from_utf8_lossy(&output.stderr));
}

/// Run a git command
pub fn git_command(repo_path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(args)
        .output()
        .expect("Failed to run git command");
    
    assert!(output.status.success(), "git command failed: {}", String::from_utf8_lossy(&output.stderr));
}

/// Run ca command and return output (uses target/debug/ca binary)
pub fn run_ca_command(repo_path: &Path, args: &[&str]) -> std::process::Output {
    // Use the debug binary from cargo build
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let ca_binary = Path::new(manifest_dir).join("target/debug/ca");
    
    Command::new(&ca_binary)
        .current_dir(repo_path)
        .args(args)
        .output()
        .expect("Failed to run ca command - make sure 'cargo build' has been run")
}
