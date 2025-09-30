use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Represents a single entry in a stack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackEntry {
    /// Unique identifier for this entry
    pub id: Uuid,
    /// Branch name for this entry
    pub branch: String,
    /// Commit hash
    pub commit_hash: String,
    /// Commit message
    pub message: String,
    /// Parent entry ID (None for base)
    pub parent_id: Option<Uuid>,
    /// Child entry IDs
    pub children: Vec<Uuid>,
    /// When this entry was created
    pub created_at: DateTime<Utc>,
    /// When this entry was last updated
    pub updated_at: DateTime<Utc>,
    /// Whether this entry has been submitted for review
    pub is_submitted: bool,
    /// Pull request ID if submitted
    pub pull_request_id: Option<String>,
    /// Whether this entry is synced with remote
    pub is_synced: bool,
}

/// Represents the status of a stack
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StackStatus {
    /// Stack is clean and ready
    Clean,
    /// Stack has uncommitted changes
    Dirty,
    /// Stack needs to be synced with remote
    OutOfSync,
    /// Stack has conflicts that need resolution
    Conflicted,
    /// Stack is being rebased
    Rebasing,
    /// Stack needs sync due to new commits on base branch
    NeedsSync,
    /// Stack has corrupted or missing commits
    Corrupted,
}

/// Represents a complete stack of commits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stack {
    /// Unique identifier for this stack
    pub id: Uuid,
    /// Human-readable name for the stack
    pub name: String,
    /// Description of what this stack implements
    pub description: Option<String>,
    /// Base branch this stack is built on
    pub base_branch: String,
    /// Working branch where commits are made (e.g., feature-1)
    pub working_branch: Option<String>,
    /// All entries in this stack (ordered)
    pub entries: Vec<StackEntry>,
    /// Map of entry ID to entry for quick lookup
    pub entry_map: HashMap<Uuid, StackEntry>,
    /// Current status of the stack
    pub status: StackStatus,
    /// When this stack was created
    pub created_at: DateTime<Utc>,
    /// When this stack was last updated
    pub updated_at: DateTime<Utc>,
    /// Whether this stack is active (current working stack)
    pub is_active: bool,
}

impl Stack {
    /// Create a new empty stack
    pub fn new(name: String, base_branch: String, description: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            base_branch,
            working_branch: None,
            entries: Vec::new(),
            entry_map: HashMap::new(),
            status: StackStatus::Clean,
            created_at: now,
            updated_at: now,
            is_active: false,
        }
    }

    /// Add a new entry to the top of the stack
    pub fn push_entry(&mut self, branch: String, commit_hash: String, message: String) -> Uuid {
        let now = Utc::now();
        let entry_id = Uuid::new_v4();

        // Find the current top entry to set as parent
        let parent_id = self.entries.last().map(|entry| entry.id);

        let entry = StackEntry {
            id: entry_id,
            branch,
            commit_hash,
            message,
            parent_id,
            children: Vec::new(),
            created_at: now,
            updated_at: now,
            is_submitted: false,
            pull_request_id: None,
            is_synced: false,
        };

        // Update parent's children if exists
        if let Some(parent_id) = parent_id {
            if let Some(parent) = self.entry_map.get_mut(&parent_id) {
                parent.children.push(entry_id);
            }
        }

        // Add to collections
        self.entries.push(entry.clone());
        self.entry_map.insert(entry_id, entry);
        self.updated_at = now;

        entry_id
    }

    /// Remove the top entry from the stack
    pub fn pop_entry(&mut self) -> Option<StackEntry> {
        if let Some(entry) = self.entries.pop() {
            let entry_id = entry.id;
            self.entry_map.remove(&entry_id);

            // Update parent's children if exists
            if let Some(parent_id) = entry.parent_id {
                if let Some(parent) = self.entry_map.get_mut(&parent_id) {
                    parent.children.retain(|&id| id != entry_id);
                }
            }

            self.updated_at = Utc::now();
            Some(entry)
        } else {
            None
        }
    }

    /// Get an entry by ID
    pub fn get_entry(&self, id: &Uuid) -> Option<&StackEntry> {
        self.entry_map.get(id)
    }

    /// Get a mutable entry by ID
    pub fn get_entry_mut(&mut self, id: &Uuid) -> Option<&mut StackEntry> {
        self.entry_map.get_mut(id)
    }

    /// Get the base (first) entry of the stack
    pub fn get_base_entry(&self) -> Option<&StackEntry> {
        self.entries.first()
    }

    /// Get the top (last) entry of the stack
    pub fn get_top_entry(&self) -> Option<&StackEntry> {
        self.entries.last()
    }

    /// Get all entries that are children of the given entry
    pub fn get_children(&self, entry_id: &Uuid) -> Vec<&StackEntry> {
        if let Some(entry) = self.get_entry(entry_id) {
            entry
                .children
                .iter()
                .filter_map(|id| self.get_entry(id))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get the parent of the given entry
    pub fn get_parent(&self, entry_id: &Uuid) -> Option<&StackEntry> {
        if let Some(entry) = self.get_entry(entry_id) {
            entry
                .parent_id
                .and_then(|parent_id| self.get_entry(&parent_id))
        } else {
            None
        }
    }

    /// Check if the stack is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the number of entries in the stack
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Mark an entry as submitted with a pull request ID
    pub fn mark_entry_submitted(&mut self, entry_id: &Uuid, pull_request_id: String) -> bool {
        if let Some(entry) = self.get_entry_mut(entry_id) {
            entry.is_submitted = true;
            entry.pull_request_id = Some(pull_request_id);
            entry.updated_at = Utc::now();
            self.updated_at = Utc::now();

            // Synchronize the entries vector with the updated entry_map
            self.sync_entries_from_map();
            true
        } else {
            false
        }
    }

    /// Synchronize the entries vector with the entry_map (entry_map is source of truth)
    fn sync_entries_from_map(&mut self) {
        for entry in &mut self.entries {
            if let Some(updated_entry) = self.entry_map.get(&entry.id) {
                *entry = updated_entry.clone();
            }
        }
    }

    /// Force synchronization of entries from entry_map (public method for fixing corrupted data)
    pub fn repair_data_consistency(&mut self) {
        self.sync_entries_from_map();
    }

    /// Mark an entry as synced
    pub fn mark_entry_synced(&mut self, entry_id: &Uuid) -> bool {
        if let Some(entry) = self.get_entry_mut(entry_id) {
            entry.is_synced = true;
            entry.updated_at = Utc::now();
            self.updated_at = Utc::now();

            // Synchronize the entries vector with the updated entry_map
            self.sync_entries_from_map();
            true
        } else {
            false
        }
    }

    /// Update stack status
    pub fn update_status(&mut self, status: StackStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }

    /// Set this stack as active
    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
        self.updated_at = Utc::now();
    }

    /// Get all branch names in this stack
    pub fn get_branch_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.branch.clone())
            .collect()
    }

    /// Validate the stack structure and Git state integrity
    pub fn validate(&self) -> Result<String, String> {
        // Validate basic structure
        if self.entries.is_empty() {
            return Ok("Empty stack is valid".to_string());
        }

        // Check parent-child relationships
        for (i, entry) in self.entries.iter().enumerate() {
            if i == 0 {
                // First entry should have no parent
                if entry.parent_id.is_some() {
                    return Err(format!(
                        "First entry {} should not have a parent",
                        entry.short_hash()
                    ));
                }
            } else {
                // Other entries should have the previous entry as parent
                let expected_parent = &self.entries[i - 1];
                if entry.parent_id != Some(expected_parent.id) {
                    return Err(format!(
                        "Entry {} has incorrect parent relationship",
                        entry.short_hash()
                    ));
                }
            }

            // Check if parent exists in map
            if let Some(parent_id) = entry.parent_id {
                if !self.entry_map.contains_key(&parent_id) {
                    return Err(format!(
                        "Entry {} references non-existent parent {}",
                        entry.short_hash(),
                        parent_id
                    ));
                }
            }
        }

        // Check that all entries are in the map
        for entry in &self.entries {
            if !self.entry_map.contains_key(&entry.id) {
                return Err(format!(
                    "Entry {} is not in the entry map",
                    entry.short_hash()
                ));
            }
        }

        // Check for duplicate IDs
        let mut seen_ids = std::collections::HashSet::new();
        for entry in &self.entries {
            if !seen_ids.insert(entry.id) {
                return Err(format!("Duplicate entry ID: {}", entry.id));
            }
        }

        // Check for duplicate branch names
        let mut seen_branches = std::collections::HashSet::new();
        for entry in &self.entries {
            if !seen_branches.insert(&entry.branch) {
                return Err(format!("Duplicate branch name: {}", entry.branch));
            }
        }

        Ok("Stack validation passed".to_string())
    }

    /// Validate Git state integrity (requires Git repository access)
    /// This checks that branch HEADs match the expected commit hashes
    pub fn validate_git_integrity(
        &self,
        git_repo: &crate::git::GitRepository,
    ) -> Result<String, String> {
        use tracing::warn;

        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        for entry in &self.entries {
            // Check if branch exists
            if !git_repo.branch_exists(&entry.branch) {
                issues.push(format!(
                    "Branch '{}' for entry {} does not exist",
                    entry.branch,
                    entry.short_hash()
                ));
                continue;
            }

            // Check if branch HEAD matches stored commit hash
            match git_repo.get_branch_head(&entry.branch) {
                Ok(branch_head) => {
                    if branch_head != entry.commit_hash {
                        issues.push(format!(
                            "🚨 BRANCH MODIFICATION DETECTED: Branch '{}' has been manually modified!\n   \
                             Expected commit: {} (from stack entry)\n   \
                             Actual commit:   {} (current branch HEAD)\n   \
                             💡 Someone may have checked out '{}' and added commits.\n   \
                             This breaks stack integrity!",
                            entry.branch,
                            &entry.commit_hash[..8],
                            &branch_head[..8],
                            entry.branch
                        ));
                    }
                }
                Err(e) => {
                    warnings.push(format!(
                        "Could not check branch '{}' HEAD: {}",
                        entry.branch, e
                    ));
                }
            }

            // Check if commit still exists
            match git_repo.commit_exists(&entry.commit_hash) {
                Ok(exists) => {
                    if !exists {
                        issues.push(format!(
                            "Commit {} for entry {} no longer exists",
                            entry.short_hash(),
                            entry.id
                        ));
                    }
                }
                Err(e) => {
                    warnings.push(format!(
                        "Could not verify commit {} existence: {}",
                        entry.short_hash(),
                        e
                    ));
                }
            }
        }

        // Log warnings
        for warning in &warnings {
            warn!("{}", warning);
        }

        if !issues.is_empty() {
            Err(format!(
                "Git integrity validation failed:\n{}{}",
                issues.join("\n"),
                if !warnings.is_empty() {
                    format!("\n\nWarnings:\n{}", warnings.join("\n"))
                } else {
                    String::new()
                }
            ))
        } else if !warnings.is_empty() {
            Ok(format!(
                "Git integrity validation passed with warnings:\n{}",
                warnings.join("\n")
            ))
        } else {
            Ok("Git integrity validation passed".to_string())
        }
    }
}

impl StackEntry {
    /// Check if this entry can be safely modified
    pub fn can_modify(&self) -> bool {
        !self.is_submitted && !self.is_synced
    }

    /// Get a short version of the commit hash
    pub fn short_hash(&self) -> String {
        if self.commit_hash.len() >= 8 {
            self.commit_hash[..8].to_string()
        } else {
            self.commit_hash.clone()
        }
    }

    /// Get a short version of the commit message
    pub fn short_message(&self, max_len: usize) -> String {
        let trimmed = self.message.trim();
        if trimmed.len() > max_len {
            format!("{}...", &trimmed[..max_len])
        } else {
            trimmed.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_empty_stack() {
        let stack = Stack::new(
            "test-stack".to_string(),
            "main".to_string(),
            Some("Test stack description".to_string()),
        );

        assert_eq!(stack.name, "test-stack");
        assert_eq!(stack.base_branch, "main");
        assert_eq!(
            stack.description,
            Some("Test stack description".to_string())
        );
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
        assert_eq!(stack.status, StackStatus::Clean);
        assert!(!stack.is_active);
    }

    #[test]
    fn test_push_pop_entries() {
        let mut stack = Stack::new("test".to_string(), "main".to_string(), None);

        // Push first entry
        let entry1_id = stack.push_entry(
            "feature-1".to_string(),
            "abc123".to_string(),
            "Add feature 1".to_string(),
        );

        assert_eq!(stack.len(), 1);
        assert!(!stack.is_empty());

        let entry1 = stack.get_entry(&entry1_id).unwrap();
        assert_eq!(entry1.branch, "feature-1");
        assert_eq!(entry1.commit_hash, "abc123");
        assert_eq!(entry1.message, "Add feature 1");
        assert_eq!(entry1.parent_id, None);
        assert!(entry1.children.is_empty());

        // Push second entry
        let entry2_id = stack.push_entry(
            "feature-2".to_string(),
            "def456".to_string(),
            "Add feature 2".to_string(),
        );

        assert_eq!(stack.len(), 2);

        let entry2 = stack.get_entry(&entry2_id).unwrap();
        assert_eq!(entry2.parent_id, Some(entry1_id));

        // Check parent-child relationship
        let updated_entry1 = stack.get_entry(&entry1_id).unwrap();
        assert_eq!(updated_entry1.children, vec![entry2_id]);

        // Pop entry
        let popped = stack.pop_entry().unwrap();
        assert_eq!(popped.id, entry2_id);
        assert_eq!(stack.len(), 1);

        // Check parent's children were updated
        let updated_entry1 = stack.get_entry(&entry1_id).unwrap();
        assert!(updated_entry1.children.is_empty());
    }

    #[test]
    fn test_stack_navigation() {
        let mut stack = Stack::new("test".to_string(), "main".to_string(), None);

        let entry1_id = stack.push_entry(
            "branch1".to_string(),
            "hash1".to_string(),
            "msg1".to_string(),
        );
        let entry2_id = stack.push_entry(
            "branch2".to_string(),
            "hash2".to_string(),
            "msg2".to_string(),
        );
        let entry3_id = stack.push_entry(
            "branch3".to_string(),
            "hash3".to_string(),
            "msg3".to_string(),
        );

        // Test base and top
        assert_eq!(stack.get_base_entry().unwrap().id, entry1_id);
        assert_eq!(stack.get_top_entry().unwrap().id, entry3_id);

        // Test parent/child relationships
        assert_eq!(stack.get_parent(&entry2_id).unwrap().id, entry1_id);
        assert_eq!(stack.get_parent(&entry3_id).unwrap().id, entry2_id);
        assert!(stack.get_parent(&entry1_id).is_none());

        let children_of_1 = stack.get_children(&entry1_id);
        assert_eq!(children_of_1.len(), 1);
        assert_eq!(children_of_1[0].id, entry2_id);
    }

    #[test]
    fn test_stack_validation() {
        let mut stack = Stack::new("test".to_string(), "main".to_string(), None);

        // Empty stack should be valid
        assert!(stack.validate().is_ok());

        // Add some entries
        stack.push_entry(
            "branch1".to_string(),
            "hash1".to_string(),
            "msg1".to_string(),
        );
        stack.push_entry(
            "branch2".to_string(),
            "hash2".to_string(),
            "msg2".to_string(),
        );

        // Valid stack should pass validation
        let result = stack.validate();
        assert!(result.is_ok());
        assert!(result.unwrap().contains("validation passed"));
    }

    #[test]
    fn test_mark_entry_submitted() {
        let mut stack = Stack::new("test".to_string(), "main".to_string(), None);
        let entry_id = stack.push_entry(
            "branch1".to_string(),
            "hash1".to_string(),
            "msg1".to_string(),
        );

        assert!(!stack.get_entry(&entry_id).unwrap().is_submitted);
        assert!(stack
            .get_entry(&entry_id)
            .unwrap()
            .pull_request_id
            .is_none());

        assert!(stack.mark_entry_submitted(&entry_id, "PR-123".to_string()));

        let entry = stack.get_entry(&entry_id).unwrap();
        assert!(entry.is_submitted);
        assert_eq!(entry.pull_request_id, Some("PR-123".to_string()));
    }

    #[test]
    fn test_branch_names() {
        let mut stack = Stack::new("test".to_string(), "main".to_string(), None);

        assert!(stack.get_branch_names().is_empty());

        stack.push_entry(
            "feature-1".to_string(),
            "hash1".to_string(),
            "msg1".to_string(),
        );
        stack.push_entry(
            "feature-2".to_string(),
            "hash2".to_string(),
            "msg2".to_string(),
        );

        let branches = stack.get_branch_names();
        assert_eq!(branches, vec!["feature-1", "feature-2"]);
    }
}
