use crate::errors::{CascadeError, Result};
use serde::Serialize;
use std::fs;
use std::path::Path;

/// Platform-specific utilities for cross-platform compatibility
pub mod platform;

/// Terminal spinner utilities for progress indication
pub mod spinner;

/// Atomic file operations to prevent corruption during writes
pub mod atomic_file {
    use super::*;

    /// Write JSON data to a file atomically using a temporary file + rename strategy with file locking
    pub fn write_json<T: Serialize>(path: &Path, data: &T) -> Result<()> {
        with_concurrent_file_lock(path, || {
            let content = serde_json::to_string_pretty(data)
                .map_err(|e| CascadeError::config(format!("Failed to serialize data: {e}")))?;

            write_string_unlocked(path, &content)
        })
    }

    /// Write string content to a file atomically using a temporary file + rename strategy with file locking
    pub fn write_string(path: &Path, content: &str) -> Result<()> {
        with_concurrent_file_lock(path, || write_string_unlocked(path, content))
    }

    /// Execute an operation with file locking optimized for concurrent access
    fn with_concurrent_file_lock<F, R>(file_path: &Path, operation: F) -> Result<R>
    where
        F: FnOnce() -> Result<R>,
    {
        // Use aggressive timeout in environments where concurrent access is expected
        let use_aggressive =
            std::env::var("CI").is_ok() || std::env::var("CONCURRENT_ACCESS_EXPECTED").is_ok();

        let _lock = if use_aggressive {
            crate::utils::file_locking::FileLock::acquire_aggressive(file_path)?
        } else {
            crate::utils::file_locking::FileLock::acquire(file_path)?
        };

        operation()
    }

    /// Internal unlocked version for use within lock contexts
    fn write_string_unlocked(path: &Path, content: &str) -> Result<()> {
        // Create temporary file in the same directory as the target
        let temp_path = path.with_extension("tmp");

        // Write to temporary file first and ensure it's synced to disk
        {
            use std::fs::File;
            use std::io::Write;

            let mut file = File::create(&temp_path).map_err(|e| {
                CascadeError::config(format!("Failed to create temporary file: {e}"))
            })?;

            file.write_all(content.as_bytes()).map_err(|e| {
                CascadeError::config(format!("Failed to write to temporary file: {e}"))
            })?;

            // Force data to be written to disk before rename
            file.sync_all().map_err(|e| {
                CascadeError::config(format!("Failed to sync temporary file to disk: {e}"))
            })?;
        }

        // Platform-specific atomic rename
        atomic_rename(&temp_path, path)
    }

    /// Platform-specific atomic rename operation
    #[cfg(windows)]
    fn atomic_rename(temp_path: &Path, final_path: &Path) -> Result<()> {
        // Windows: More robust rename with retry on failure
        const MAX_RETRIES: u32 = 3;
        const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(100);

        for attempt in 1..=MAX_RETRIES {
            match fs::rename(temp_path, final_path) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if attempt == MAX_RETRIES {
                        // Clean up temp file on final failure
                        let _ = fs::remove_file(temp_path);
                        return Err(CascadeError::config(format!(
                            "Failed to finalize file write after {MAX_RETRIES} attempts on Windows: {e}"
                        )));
                    }

                    // Retry after a short delay for transient Windows file locking issues
                    std::thread::sleep(RETRY_DELAY);
                }
            }
        }

        unreachable!("Loop should have returned or failed by now")
    }

    #[cfg(not(windows))]
    fn atomic_rename(temp_path: &Path, final_path: &Path) -> Result<()> {
        // Unix: Standard atomic rename works reliably
        fs::rename(temp_path, final_path)
            .map_err(|e| CascadeError::config(format!("Failed to finalize file write: {e}")))?;
        Ok(())
    }

    /// Write binary data to a file atomically with file locking
    pub fn write_bytes(path: &Path, data: &[u8]) -> Result<()> {
        with_concurrent_file_lock(path, || {
            let temp_path = path.with_extension("tmp");

            // Write and sync binary data to disk
            {
                use std::fs::File;
                use std::io::Write;

                let mut file = File::create(&temp_path).map_err(|e| {
                    CascadeError::config(format!("Failed to create temporary file: {e}"))
                })?;

                file.write_all(data).map_err(|e| {
                    CascadeError::config(format!("Failed to write to temporary file: {e}"))
                })?;

                // Force data to be written to disk before rename
                file.sync_all().map_err(|e| {
                    CascadeError::config(format!("Failed to sync temporary file to disk: {e}"))
                })?;
            }

            atomic_rename(&temp_path, path)
        })
    }
}

/// Path validation utilities to prevent path traversal attacks
pub mod path_validation {
    use super::*;
    use std::path::PathBuf;

    /// Validate and canonicalize a path to ensure it's within allowed boundaries
    /// Handles both existing and non-existing paths for security validation
    pub fn validate_config_path(path: &Path, base_dir: &Path) -> Result<PathBuf> {
        // For non-existing paths, we need to validate without canonicalize
        if !path.exists() {
            // Validate the base directory exists and can be canonicalized
            let canonical_base = base_dir.canonicalize().map_err(|e| {
                CascadeError::config(format!("Invalid base directory '{base_dir:?}': {e}"))
            })?;

            // For non-existing paths, check if the parent directory is within bounds
            let mut check_path = path.to_path_buf();

            // Find the first existing parent
            while !check_path.exists() && check_path.parent().is_some() {
                check_path = check_path.parent().unwrap().to_path_buf();
            }

            if check_path.exists() {
                let canonical_check = check_path.canonicalize().map_err(|e| {
                    CascadeError::config(format!("Cannot validate path security: {e}"))
                })?;

                if !canonical_check.starts_with(&canonical_base) {
                    return Err(CascadeError::config(format!(
                        "Path '{path:?}' would be outside allowed directory '{canonical_base:?}'"
                    )));
                }
            }

            // Return the original path for non-existing files
            Ok(path.to_path_buf())
        } else {
            // For existing paths, use full canonicalization
            let canonical_path = path
                .canonicalize()
                .map_err(|e| CascadeError::config(format!("Invalid path '{path:?}': {e}")))?;

            let canonical_base = base_dir.canonicalize().map_err(|e| {
                CascadeError::config(format!("Invalid base directory '{base_dir:?}': {e}"))
            })?;

            if !canonical_path.starts_with(&canonical_base) {
                return Err(CascadeError::config(format!(
                    "Path '{canonical_path:?}' is outside allowed directory '{canonical_base:?}'"
                )));
            }

            Ok(canonical_path)
        }
    }

    /// Sanitize a filename to prevent issues with special characters
    pub fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
                _ => '_',
            })
            .collect()
    }
}

/// Async utilities to prevent blocking operations
pub mod async_ops {
    use super::*;
    use tokio::task;

    /// Run a potentially blocking Git operation in a background thread
    pub async fn run_git_operation<F, R>(operation: F) -> Result<R>
    where
        F: FnOnce() -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        task::spawn_blocking(operation)
            .await
            .map_err(|e| CascadeError::config(format!("Background task failed: {e}")))?
    }

    /// Run a potentially blocking file operation in a background thread
    pub async fn run_file_operation<F, R>(operation: F) -> Result<R>
    where
        F: FnOnce() -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        task::spawn_blocking(operation)
            .await
            .map_err(|e| CascadeError::config(format!("File operation failed: {e}")))?
    }
}

/// Git lock utilities for detecting and cleaning stale locks
pub mod git_lock {
    use super::*;
    use std::path::Path;
    use std::process::Command;

    /// Check if any Git processes are currently running
    /// Returns Some(true) if Git processes detected, Some(false) if none detected,
    /// None if detection failed (unable to check)
    fn has_running_git_processes() -> Option<bool> {
        // Try to detect running git processes
        #[cfg(unix)]
        {
            match Command::new("pgrep").arg("-i").arg("git").output() {
                Ok(output) => {
                    return Some(!output.stdout.is_empty());
                }
                Err(e) => {
                    tracing::debug!("Failed to run pgrep to detect Git processes: {}", e);
                    return None;
                }
            }
        }

        #[cfg(windows)]
        {
            match Command::new("tasklist")
                .arg("/FI")
                .arg("IMAGENAME eq git.exe")
                .output()
            {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return Some(stdout.contains("git.exe"));
                }
                Err(e) => {
                    tracing::debug!("Failed to run tasklist to detect Git processes: {}", e);
                    return None;
                }
            }
        }

        #[allow(unreachable_code)]
        {
            // Fallback for non-unix, non-windows platforms
            None
        }
    }

    /// Detect and remove stale Git index lock if safe to do so
    /// Returns true if a stale lock was removed
    pub fn clean_stale_index_lock(repo_path: &Path) -> Result<bool> {
        let lock_path = crate::git::resolve_git_dir(repo_path)?.join("index.lock");

        if !lock_path.exists() {
            return Ok(false);
        }

        // Check if any Git processes are running
        match has_running_git_processes() {
            Some(true) => {
                // Git processes are definitely running, don't remove the lock
                tracing::debug!(
                    "Git index lock exists and Git processes are running - not removing"
                );
                Ok(false)
            }
            Some(false) => {
                // No Git processes detected - safe to remove stale lock
                tracing::debug!(
                    "Detected stale Git index lock (no Git processes running), removing: {:?}",
                    lock_path
                );

                fs::remove_file(&lock_path).map_err(|e| {
                    CascadeError::config(format!(
                        "Failed to remove stale index lock at {:?}: {}",
                        lock_path, e
                    ))
                })?;

                tracing::info!("Removed stale Git index lock at {:?}", lock_path);
                Ok(true)
            }
            None => {
                // Can't detect processes - fail safe by not removing the lock
                tracing::debug!(
                    "Cannot detect Git processes (pgrep/tasklist unavailable) - not removing lock for safety"
                );
                Ok(false)
            }
        }
    }

    /// Check if an error is a Git lock error
    pub fn is_lock_error(error: &git2::Error) -> bool {
        error.code() == git2::ErrorCode::Locked
            || error.message().contains("index is locked")
            || error.message().contains("index.lock")
    }

    /// Execute a git2 operation with exponential-backoff retry on index lock contention.
    /// Retries up to 4 times with delays of 50ms, 100ms, 200ms before falling back
    /// to stale lock cleanup as a last resort.
    pub fn with_lock_retry<F, R>(repo_path: &Path, operation: F) -> Result<R>
    where
        F: Fn() -> std::result::Result<R, git2::Error>,
    {
        const MAX_ATTEMPTS: u32 = 4;
        const BASE_DELAY_MS: u64 = 50;

        let mut last_error = None;

        for attempt in 0..MAX_ATTEMPTS {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) if is_lock_error(&e) => {
                    last_error = Some(e);
                    if attempt < MAX_ATTEMPTS - 1 {
                        let delay_ms = BASE_DELAY_MS * 2_u64.pow(attempt);
                        tracing::debug!(
                            "Index lock contention (attempt {}/{}), retrying in {}ms",
                            attempt + 1,
                            MAX_ATTEMPTS,
                            delay_ms,
                        );
                        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    }
                }
                Err(e) => return Err(CascadeError::Git(e)),
            }
        }

        // Last resort: try stale lock cleanup
        if let Ok(true) = clean_stale_index_lock(repo_path) {
            tracing::info!("Removed stale lock after retries exhausted, final attempt");
            return operation().map_err(CascadeError::Git);
        }

        Err(CascadeError::Git(last_error.unwrap()))
    }

    /// Wait for the git index lock to be released before starting a destructive operation.
    /// Polls the filesystem for the lock file to disappear. IDE locks are transient
    /// (milliseconds for a status check), so a short timeout catches the common case.
    /// Falls back to stale lock cleanup if the lock persists past the timeout.
    pub fn wait_for_index_lock(repo_path: &Path, timeout: std::time::Duration) -> Result<()> {
        let lock_path = crate::git::resolve_git_dir(repo_path)?.join("index.lock");

        if !lock_path.exists() {
            return Ok(());
        }

        tracing::debug!("Index lock detected, waiting for it to clear...");

        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(50);

        while start.elapsed() < timeout {
            std::thread::sleep(poll_interval);
            if !lock_path.exists() {
                tracing::debug!("Index lock cleared after {:?}", start.elapsed());
                return Ok(());
            }
        }

        // Lock persisted past timeout — try stale lock cleanup as last resort
        if let Ok(true) = clean_stale_index_lock(repo_path) {
            tracing::info!("Removed stale index lock after timeout");
            return Ok(());
        }

        Err(CascadeError::branch(format!(
            "Git index is locked ({}).\n\n\
             Another program is using Git in this repository.\n\n\
             Common causes:\n\
             \u{2022} An IDE with Git integration has this repo open\n\
             \u{2022} Another terminal is running a git command\n\
             \u{2022} A previous git operation crashed and left a stale lock\n\n\
             To fix:\n\
             1. Close any IDEs or Git-aware tools using this repo, then retry\n\
             2. If no Git processes are running: rm -f {}\n\
             3. Check for running git processes: pgrep -l git",
            lock_path.display(),
            lock_path.display(),
        )))
    }

    /// Retry an operation that returns CascadeError on index lock contention.
    /// Uses exponential backoff with the same timing as with_lock_retry.
    pub fn retry_on_lock<F, R>(max_attempts: u32, operation: F) -> Result<R>
    where
        F: Fn() -> Result<R>,
    {
        let mut last_error = None;
        for attempt in 0..max_attempts {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) if e.is_lock_error() && attempt < max_attempts - 1 => {
                    let delay = 50 * 2u64.pow(attempt);
                    tracing::debug!(
                        "Operation hit index lock (attempt {}/{}), retry in {}ms",
                        attempt + 1,
                        max_attempts,
                        delay,
                    );
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                    last_error = Some(e);
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_error.unwrap())
    }
}

/// File locking utilities for concurrent access protection
pub mod file_locking {
    use super::*;
    use std::fs::{File, OpenOptions};
    use std::path::Path;
    use std::time::{Duration, Instant};

    /// A file lock that prevents concurrent access to critical files
    pub struct FileLock {
        _file: File,
        lock_path: std::path::PathBuf,
    }

    impl FileLock {
        /// Platform-specific configuration for file locking
        #[cfg(windows)]
        const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10); // Longer timeout for Windows
        #[cfg(windows)]
        const RETRY_INTERVAL: Duration = Duration::from_millis(100); // Less aggressive polling

        #[cfg(not(windows))]
        const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5); // Shorter timeout for Unix
        #[cfg(not(windows))]
        const RETRY_INTERVAL: Duration = Duration::from_millis(50); // More frequent polling

        /// Attempt to acquire a lock on a file with timeout
        pub fn acquire_with_timeout(file_path: &Path, timeout: Duration) -> Result<Self> {
            let lock_path = file_path.with_extension("lock");
            let start_time = Instant::now();

            loop {
                match Self::try_acquire(&lock_path) {
                    Ok(lock) => return Ok(lock),
                    Err(e) => {
                        if start_time.elapsed() >= timeout {
                            return Err(CascadeError::config(format!(
                                "Timeout waiting for lock on {file_path:?} after {}ms (platform: {}): {e}",
                                timeout.as_millis(),
                                if cfg!(windows) { "windows" } else { "unix" }
                            )));
                        }
                        std::thread::sleep(Self::RETRY_INTERVAL);
                    }
                }
            }
        }

        /// Try to acquire a lock immediately (non-blocking)
        pub fn try_acquire(lock_path: &Path) -> Result<Self> {
            // Platform-specific lock file creation
            let file = Self::create_lock_file(lock_path)?;

            Ok(Self {
                _file: file,
                lock_path: lock_path.to_path_buf(),
            })
        }

        /// Platform-specific lock file creation
        #[cfg(windows)]
        fn create_lock_file(lock_path: &Path) -> Result<File> {
            // Windows: More robust file creation with explicit sharing mode
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(lock_path)
                .map_err(|e| {
                    // Provide more specific error information for Windows
                    match e.kind() {
                        std::io::ErrorKind::AlreadyExists => {
                            CascadeError::config(format!(
                                "Lock file {lock_path:?} already exists - another process may be accessing the file"
                            ))
                        }
                        std::io::ErrorKind::PermissionDenied => {
                            CascadeError::config(format!(
                                "Permission denied creating lock file {lock_path:?} - check file permissions"
                            ))
                        }
                        _ => CascadeError::config(format!(
                            "Failed to acquire lock {lock_path:?} on Windows: {e}"
                        ))
                    }
                })
        }

        #[cfg(not(windows))]
        fn create_lock_file(lock_path: &Path) -> Result<File> {
            // Unix: Standard approach works well
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(lock_path)
                .map_err(|e| {
                    CascadeError::config(format!("Failed to acquire lock {lock_path:?}: {e}"))
                })
        }

        /// Acquire a lock with platform-appropriate default timeout
        pub fn acquire(file_path: &Path) -> Result<Self> {
            Self::acquire_with_timeout(file_path, Self::DEFAULT_TIMEOUT)
        }

        /// Acquire a lock with aggressive timeout for high-concurrency scenarios
        pub fn acquire_aggressive(file_path: &Path) -> Result<Self> {
            let timeout = if cfg!(windows) {
                Duration::from_secs(15) // Even longer for Windows under load
            } else {
                Duration::from_secs(8) // Slightly longer for Unix under load
            };
            Self::acquire_with_timeout(file_path, timeout)
        }
    }

    impl Drop for FileLock {
        fn drop(&mut self) {
            // Clean up lock file on drop
            let _ = std::fs::remove_file(&self.lock_path);
        }
    }

    /// Execute an operation with file locking protection
    pub fn with_file_lock<F, R>(file_path: &Path, operation: F) -> Result<R>
    where
        F: FnOnce() -> Result<R>,
    {
        let _lock = FileLock::acquire(file_path)?;
        operation()
    }

    /// Execute an async operation with file locking protection
    pub async fn with_file_lock_async<F, Fut, R>(file_path: &Path, operation: F) -> Result<R>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<R>>,
    {
        let file_path = file_path.to_path_buf();
        let _lock = tokio::task::spawn_blocking(move || FileLock::acquire(&file_path))
            .await
            .map_err(|e| CascadeError::config(format!("Lock task failed: {e}")))?;

        operation().await
    }
}
