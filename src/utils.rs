use crate::errors::{CascadeError, Result};
use serde::Serialize;
use std::fs;
use std::path::Path;

/// Atomic file operations to prevent corruption during writes
pub mod atomic_file {
    use super::*;

    /// Write JSON data to a file atomically using a temporary file + rename strategy
    pub fn write_json<T: Serialize>(path: &Path, data: &T) -> Result<()> {
        let content = serde_json::to_string_pretty(data)
            .map_err(|e| CascadeError::config(format!("Failed to serialize data: {e}")))?;

        write_string(path, &content)
    }

    /// Write string content to a file atomically using a temporary file + rename strategy
    pub fn write_string(path: &Path, content: &str) -> Result<()> {
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

    /// Write binary data to a file atomically
    pub fn write_bytes(path: &Path, data: &[u8]) -> Result<()> {
        let temp_path = path.with_extension("tmp");

        fs::write(&temp_path, data)
            .map_err(|e| CascadeError::config(format!("Failed to write temporary file: {e}")))?;

        fs::rename(&temp_path, path)
            .map_err(|e| CascadeError::config(format!("Failed to finalize file write: {e}")))?;

        Ok(())
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
            let canonical_base = base_dir.canonicalize()
                .map_err(|e| CascadeError::config(format!("Invalid base directory '{:?}': {e}", base_dir)))?;

            // For non-existing paths, check if the parent directory is within bounds
            let mut check_path = path.to_path_buf();
            
            // Find the first existing parent
            while !check_path.exists() && check_path.parent().is_some() {
                check_path = check_path.parent().unwrap().to_path_buf();
            }

            if check_path.exists() {
                let canonical_check = check_path.canonicalize()
                    .map_err(|e| CascadeError::config(format!("Cannot validate path security: {e}")))?;
                
                if !canonical_check.starts_with(&canonical_base) {
                    return Err(CascadeError::config(format!(
                        "Path '{:?}' would be outside allowed directory '{:?}'",
                        path, canonical_base
                    )));
                }
            }

            // Return the original path for non-existing files
            Ok(path.to_path_buf())
        } else {
            // For existing paths, use full canonicalization
            let canonical_path = path.canonicalize()
                .map_err(|e| CascadeError::config(format!("Invalid path '{:?}': {e}", path)))?;

            let canonical_base = base_dir.canonicalize()
                .map_err(|e| CascadeError::config(format!("Invalid base directory '{:?}': {e}", base_dir)))?;

            if !canonical_path.starts_with(&canonical_base) {
                return Err(CascadeError::config(format!(
                    "Path '{:?}' is outside allowed directory '{:?}'",
                    canonical_path, canonical_base
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