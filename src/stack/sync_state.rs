use crate::errors::{CascadeError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// State for an in-progress sync operation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncState {
    /// ID of the stack being synced
    pub stack_id: String,
    /// Name of the stack
    pub stack_name: String,
    /// Branch user was on before sync started
    pub original_branch: String,
    /// Base branch being rebased onto
    pub target_base: String,
    /// Entry IDs that still need to be processed
    pub remaining_entry_ids: Vec<String>,
    /// Entry currently being cherry-picked (the one with conflicts)
    pub current_entry_id: String,
    /// Current entry's branch name
    pub current_entry_branch: String,
    /// Temp branch for current entry
    pub current_temp_branch: String,
    /// All temp branches created so far (for cleanup)
    pub temp_branches: Vec<String>,
}

impl SyncState {
    /// Save sync state to disk
    pub fn save(&self, repo_root: &Path) -> Result<()> {
        let state_path = repo_root.join(".git").join("CASCADE_SYNC_STATE");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| CascadeError::config(format!("Failed to serialize sync state: {e}")))?;

        std::fs::write(&state_path, json)
            .map_err(|e| CascadeError::config(format!("Failed to write sync state: {e}")))?;

        tracing::debug!("Saved sync state to {:?}", state_path);
        Ok(())
    }

    /// Load sync state from disk
    pub fn load(repo_root: &Path) -> Result<Self> {
        let state_path = repo_root.join(".git").join("CASCADE_SYNC_STATE");

        if !state_path.exists() {
            return Err(CascadeError::config(
                "No in-progress sync found. Nothing to continue.".to_string(),
            ));
        }

        let json = std::fs::read_to_string(&state_path)
            .map_err(|e| CascadeError::config(format!("Failed to read sync state: {e}")))?;

        let state: Self = serde_json::from_str(&json)
            .map_err(|e| CascadeError::config(format!("Failed to parse sync state: {e}")))?;

        tracing::debug!("Loaded sync state from {:?}", state_path);
        Ok(state)
    }

    /// Delete sync state file
    pub fn delete(repo_root: &Path) -> Result<()> {
        let state_path = repo_root.join(".git").join("CASCADE_SYNC_STATE");

        if state_path.exists() {
            std::fs::remove_file(&state_path)
                .map_err(|e| CascadeError::config(format!("Failed to delete sync state: {e}")))?;
            tracing::debug!("Deleted sync state file");
        }

        Ok(())
    }

    /// Check if sync state exists
    pub fn exists(repo_root: &Path) -> bool {
        repo_root.join(".git").join("CASCADE_SYNC_STATE").exists()
    }
}
