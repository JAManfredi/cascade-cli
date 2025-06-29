use crate::errors::{CascadeError, Result};
use serde::Serialize;
use std::fs;
use std::path::Path;

/// Atomic file operations to prevent corruption during writes
pub mod atomic_file {
    use super::*;

    /// Write JSON data to a file atomically using a temporary file + rename strategy with file locking
    pub fn write_json<T: Serialize>(path: &Path, data: &T) -> Result<()> {
        crate::utils::file_locking::with_file_lock(path, || {
            let content = serde_json::to_string_pretty(data)
                .map_err(|e| CascadeError::config(format!("Failed to serialize data: {e}")))?;

            write_string_unlocked(path, &content)
        })
    }

    /// Write string content to a file atomically using a temporary file + rename strategy with file locking
    pub fn write_string(path: &Path, content: &str) -> Result<()> {
        crate::utils::file_locking::with_file_lock(path, || write_string_unlocked(path, content))
    }

    /// Internal unlocked version for use within lock contexts
    fn write_string_unlocked(path: &Path, content: &str) -> Result<()> {
        // Create temporary file in the same directory as the target
        let temp_path = path.with_extension("tmp");

        // Write to temporary file first
        fs::write(&temp_path, content)
            .map_err(|e| CascadeError::config(format!("Failed to write temporary file: {e}")))?;

        // Atomically rename temporary file to final destination
        fs::rename(&temp_path, path)
            .map_err(|e| CascadeError::config(format!("Failed to finalize file write: {e}")))?;

        Ok(())
    }

    /// Write binary data to a file atomically with file locking
    pub fn write_bytes(path: &Path, data: &[u8]) -> Result<()> {
        crate::utils::file_locking::with_file_lock(path, || {
            let temp_path = path.with_extension("tmp");

            fs::write(&temp_path, data).map_err(|e| {
                CascadeError::config(format!("Failed to write temporary file: {e}"))
            })?;

            fs::rename(&temp_path, path)
                .map_err(|e| CascadeError::config(format!("Failed to finalize file write: {e}")))?;

            Ok(())
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
        /// Attempt to acquire a lock on a file with timeout
        pub fn acquire_with_timeout(file_path: &Path, timeout: Duration) -> Result<Self> {
            let lock_path = file_path.with_extension("lock");
            let start_time = Instant::now();

            loop {
                match Self::try_acquire(&lock_path) {
                    Ok(lock) => return Ok(lock),
                    Err(_) => {
                        if start_time.elapsed() >= timeout {
                            return Err(CascadeError::config(format!(
                                "Timeout waiting for lock on {file_path:?}"
                            )));
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        }

        /// Try to acquire a lock immediately (non-blocking)
        pub fn try_acquire(lock_path: &Path) -> Result<Self> {
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(lock_path)
                .map_err(|e| {
                    CascadeError::config(format!("Failed to acquire lock {lock_path:?}: {e}"))
                })?;

            Ok(Self {
                _file: file,
                lock_path: lock_path.to_path_buf(),
            })
        }

        /// Acquire a lock with default timeout (5 seconds)
        pub fn acquire(file_path: &Path) -> Result<Self> {
            Self::acquire_with_timeout(file_path, Duration::from_secs(5))
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
