use crate::errors::{CascadeError, Result};
use crate::git::GitRepository;
use crate::config::get_repo_config_dir;
use super::{Stack, StackEntry, StackStatus, StackMetadata, CommitMetadata};
use super::metadata::RepositoryMetadata;
use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use uuid::Uuid;

/// Manages all stack operations and persistence
pub struct StackManager {
    /// Git repository interface
    repo: GitRepository,
    /// Path to the repository root
    repo_path: PathBuf,
    /// Path to cascade config directory
    config_dir: PathBuf,
    /// Path to stacks data file
    stacks_file: PathBuf,
    /// Path to metadata file
    metadata_file: PathBuf,
    /// In-memory stack data
    stacks: HashMap<Uuid, Stack>,
    /// Repository metadata
    metadata: RepositoryMetadata,
}

impl StackManager {
    /// Create a new StackManager for the given repository
    pub fn new(repo_path: &Path) -> Result<Self> {
        let repo = GitRepository::open(repo_path)?;
        let config_dir = get_repo_config_dir(repo_path)?;
        let stacks_file = config_dir.join("stacks.json");
        let metadata_file = config_dir.join("metadata.json");

        // Determine default base branch
        let default_base = repo.get_current_branch()
            .unwrap_or_else(|_| "main".to_string());

        let mut manager = Self {
            repo,
            repo_path: repo_path.to_path_buf(),
            config_dir,
            stacks_file,
            metadata_file,
            stacks: HashMap::new(),
            metadata: RepositoryMetadata::new(default_base),
        };

        // Load existing data if available
        manager.load_from_disk()?;

        Ok(manager)
    }

    /// Create a new stack
    pub fn create_stack(&mut self, name: String, base_branch: Option<String>, description: Option<String>) -> Result<Uuid> {
        // Check if stack with this name already exists
        if self.metadata.find_stack_by_name(&name).is_some() {
            return Err(CascadeError::config(format!("Stack '{}' already exists", name)));
        }

        // Use provided base branch or default
        let base_branch = base_branch.unwrap_or_else(|| self.metadata.default_base_branch.clone());

        // Verify base branch exists
        if !self.repo.branch_exists(&base_branch) {
            return Err(CascadeError::branch(format!("Base branch '{}' does not exist", base_branch)));
        }

        // Create the stack
        let stack = Stack::new(name.clone(), base_branch.clone(), description.clone());
        let stack_id = stack.id;

        // Create metadata
        let stack_metadata = StackMetadata::new(stack_id, name, base_branch, description);

        // Store in memory
        self.stacks.insert(stack_id, stack);
        self.metadata.add_stack(stack_metadata);

        // Set as active if it's the first stack
        if self.metadata.stacks.len() == 1 {
            self.set_active_stack(Some(stack_id))?;
        } else {
            // Just save to disk if not setting as active
            self.save_to_disk()?;
        }

        Ok(stack_id)
    }

    /// Get a stack by ID
    pub fn get_stack(&self, stack_id: &Uuid) -> Option<&Stack> {
        self.stacks.get(stack_id)
    }

    /// Get a mutable stack by ID
    pub fn get_stack_mut(&mut self, stack_id: &Uuid) -> Option<&mut Stack> {
        self.stacks.get_mut(stack_id)
    }

    /// Get stack by name
    pub fn get_stack_by_name(&self, name: &str) -> Option<&Stack> {
        if let Some(metadata) = self.metadata.find_stack_by_name(name) {
            self.stacks.get(&metadata.stack_id)
        } else {
            None
        }
    }

    /// Get the currently active stack
    pub fn get_active_stack(&self) -> Option<&Stack> {
        self.metadata.active_stack_id.and_then(|id| self.stacks.get(&id))
    }

    /// Get the currently active stack mutably
    pub fn get_active_stack_mut(&mut self) -> Option<&mut Stack> {
        if let Some(id) = self.metadata.active_stack_id {
            self.stacks.get_mut(&id)
        } else {
            None
        }
    }

    /// Set the active stack
    pub fn set_active_stack(&mut self, stack_id: Option<Uuid>) -> Result<()> {
        // Verify stack exists if provided
        if let Some(id) = stack_id {
            if !self.stacks.contains_key(&id) {
                return Err(CascadeError::config(format!("Stack with ID {} not found", id)));
            }
        }

        // Update active flag on stacks
        for stack in self.stacks.values_mut() {
            stack.set_active(Some(stack.id) == stack_id);
        }

        self.metadata.set_active_stack(stack_id);
        self.save_to_disk()?;

        Ok(())
    }

    /// Set active stack by name
    pub fn set_active_stack_by_name(&mut self, name: &str) -> Result<()> {
        if let Some(metadata) = self.metadata.find_stack_by_name(name) {
            self.set_active_stack(Some(metadata.stack_id))
        } else {
            Err(CascadeError::config(format!("Stack '{}' not found", name)))
        }
    }

    /// Delete a stack
    pub fn delete_stack(&mut self, stack_id: &Uuid) -> Result<Stack> {
        let stack = self.stacks.remove(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack with ID {} not found", stack_id)))?;

        // Remove metadata
        self.metadata.remove_stack(stack_id);

        // Remove all associated commit metadata
        let stack_commits: Vec<String> = self.metadata.commits.values()
            .filter(|commit| &commit.stack_id == stack_id)
            .map(|commit| commit.hash.clone())
            .collect();

        for commit_hash in stack_commits {
            self.metadata.remove_commit(&commit_hash);
        }

        // If this was the active stack, find a new one
        if self.metadata.active_stack_id == Some(*stack_id) {
            let new_active = self.metadata.stacks.keys().next().copied();
            self.set_active_stack(new_active)?;
        }

        self.save_to_disk()?;

        Ok(stack)
    }

    /// Push a new commit to the top of the active stack
    pub fn push_to_stack(&mut self, branch: String, commit_hash: String, message: String) -> Result<Uuid> {
        let stack_id = self.metadata.active_stack_id
            .ok_or_else(|| CascadeError::config("No active stack"))?;

        let stack = self.stacks.get_mut(&stack_id)
            .ok_or_else(|| CascadeError::config("Active stack not found"))?;

        // Verify the commit exists
        if !self.repo.commit_exists(&commit_hash)? {
            return Err(CascadeError::branch(format!("Commit {} does not exist", commit_hash)));
        }

        // Add to stack
        let entry_id = stack.push_entry(branch.clone(), commit_hash.clone(), message.clone());

        // Create commit metadata
        let commit_metadata = CommitMetadata::new(
            commit_hash.clone(),
            message,
            entry_id,
            stack_id,
            branch.clone(),
        );

        // Update repository metadata
        self.metadata.add_commit(commit_metadata);
        if let Some(stack_meta) = self.metadata.get_stack_mut(&stack_id) {
            stack_meta.add_branch(branch);
            stack_meta.add_commit(commit_hash);
        }

        self.save_to_disk()?;

        Ok(entry_id)
    }

    /// Pop the top commit from the active stack
    pub fn pop_from_stack(&mut self) -> Result<StackEntry> {
        let stack_id = self.metadata.active_stack_id
            .ok_or_else(|| CascadeError::config("No active stack"))?;

        let stack = self.stacks.get_mut(&stack_id)
            .ok_or_else(|| CascadeError::config("Active stack not found"))?;

        let entry = stack.pop_entry()
            .ok_or_else(|| CascadeError::config("Stack is empty"))?;

        // Remove commit metadata
        self.metadata.remove_commit(&entry.commit_hash);

        // Update stack metadata
        if let Some(stack_meta) = self.metadata.get_stack_mut(&stack_id) {
            stack_meta.remove_commit(&entry.commit_hash);
            // Note: We don't remove the branch as there might be other commits on it
        }

        self.save_to_disk()?;

        Ok(entry)
    }

    /// Submit a stack entry for review (mark as submitted)
    pub fn submit_entry(&mut self, stack_id: &Uuid, entry_id: &Uuid, pull_request_id: String) -> Result<()> {
        let stack = self.stacks.get_mut(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {} not found", stack_id)))?;

        let entry_commit_hash = {
            let entry = stack.get_entry(entry_id)
                .ok_or_else(|| CascadeError::config(format!("Entry {} not found", entry_id)))?;
            entry.commit_hash.clone()
        };

        // Update stack entry
        if !stack.mark_entry_submitted(entry_id, pull_request_id.clone()) {
            return Err(CascadeError::config(format!("Failed to mark entry {} as submitted", entry_id)));
        }

        // Update commit metadata
        if let Some(commit_meta) = self.metadata.commits.get_mut(&entry_commit_hash) {
            commit_meta.mark_submitted(pull_request_id);
        }

        // Update stack metadata statistics
        if let Some(stack_meta) = self.metadata.get_stack_mut(stack_id) {
            let submitted_count = stack.entries.iter()
                .filter(|e| e.is_submitted)
                .count();
            stack_meta.update_stats(stack.entries.len(), submitted_count, stack_meta.merged_commits);
        }

        self.save_to_disk()?;

        Ok(())
    }

    /// Get all stacks
    pub fn get_all_stacks(&self) -> Vec<&Stack> {
        self.stacks.values().collect()
    }

    /// Get stack metadata
    pub fn get_stack_metadata(&self, stack_id: &Uuid) -> Option<&StackMetadata> {
        self.metadata.get_stack(stack_id)
    }

    /// Get repository metadata
    pub fn get_repository_metadata(&self) -> &RepositoryMetadata {
        &self.metadata
    }

    /// Sync stack with Git repository state
    pub fn sync_stack(&mut self, stack_id: &Uuid) -> Result<()> {
        let stack = self.stacks.get_mut(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {} not found", stack_id)))?;

        // Check if all commits still exist
        let mut missing_commits = Vec::new();
        for entry in &stack.entries {
            if !self.repo.commit_exists(&entry.commit_hash)? {
                missing_commits.push(entry.commit_hash.clone());
            }
        }

        if !missing_commits.is_empty() {
            stack.update_status(StackStatus::OutOfSync);
            return Err(CascadeError::branch(format!(
                "Stack {} has missing commits: {}",
                stack.name,
                missing_commits.join(", ")
            )));
        }

        // Check if base branch has new commits
        let _base_hash = self.repo.get_branch_head(&stack.base_branch)?;
        // For now, we'll mark as clean if all commits exist
        // TODO: Implement proper base branch comparison

        stack.update_status(StackStatus::Clean);

        // Update metadata
        if let Some(stack_meta) = self.metadata.get_stack_mut(stack_id) {
            stack_meta.set_up_to_date(true);
        }

        self.save_to_disk()?;

        Ok(())
    }

    /// List all stacks with their status
    pub fn list_stacks(&self) -> Vec<(Uuid, &str, &StackStatus, usize, Option<&str>)> {
        self.stacks.values()
            .map(|stack| (
                stack.id,
                stack.name.as_str(),
                &stack.status,
                stack.entries.len(),
                if stack.is_active { Some("active") } else { None }
            ))
            .collect()
    }

    /// Validate all stacks
    pub fn validate_all(&self) -> Result<()> {
        for stack in self.stacks.values() {
            stack.validate()
                .map_err(|e| CascadeError::config(format!("Stack '{}' validation failed: {}", stack.name, e)))?;
        }
        Ok(())
    }

    /// Save all data to disk
    fn save_to_disk(&self) -> Result<()> {
        // Ensure config directory exists
        if !self.config_dir.exists() {
            fs::create_dir_all(&self.config_dir)
                .map_err(|e| CascadeError::config(format!("Failed to create config directory: {}", e)))?;
        }

        // Save stacks
        let stacks_json = serde_json::to_string_pretty(&self.stacks)
            .map_err(|e| CascadeError::config(format!("Failed to serialize stacks: {}", e)))?;

        fs::write(&self.stacks_file, stacks_json)
            .map_err(|e| CascadeError::config(format!("Failed to write stacks file: {}", e)))?;

        // Save metadata
        let metadata_json = serde_json::to_string_pretty(&self.metadata)
            .map_err(|e| CascadeError::config(format!("Failed to serialize metadata: {}", e)))?;

        fs::write(&self.metadata_file, metadata_json)
            .map_err(|e| CascadeError::config(format!("Failed to write metadata file: {}", e)))?;

        Ok(())
    }

    /// Load data from disk
    fn load_from_disk(&mut self) -> Result<()> {
        // Load stacks if file exists
        if self.stacks_file.exists() {
            let stacks_content = fs::read_to_string(&self.stacks_file)
                .map_err(|e| CascadeError::config(format!("Failed to read stacks file: {}", e)))?;

            self.stacks = serde_json::from_str(&stacks_content)
                .map_err(|e| CascadeError::config(format!("Failed to parse stacks file: {}", e)))?;
        }

        // Load metadata if file exists
        if self.metadata_file.exists() {
            let metadata_content = fs::read_to_string(&self.metadata_file)
                .map_err(|e| CascadeError::config(format!("Failed to read metadata file: {}", e)))?;

            self.metadata = serde_json::from_str(&metadata_content)
                .map_err(|e| CascadeError::config(format!("Failed to parse metadata file: {}", e)))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::process::Command;

    fn create_test_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repository
        Command::new("git")
            .args(&["init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Configure git
        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Create an initial commit
        std::fs::write(repo_path.join("README.md"), "# Test Repo").unwrap();
        Command::new("git")
            .args(&["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Initialize cascade
        crate::config::initialize_repo(&repo_path, Some("https://test.bitbucket.com".to_string())).unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_create_stack_manager() {
        let (_temp_dir, repo_path) = create_test_repo();
        let manager = StackManager::new(&repo_path).unwrap();

        assert_eq!(manager.stacks.len(), 0);
        assert!(manager.get_active_stack().is_none());
    }

    #[test]
    fn test_create_and_manage_stack() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut manager = StackManager::new(&repo_path).unwrap();

        // Create a stack using the default branch
        let stack_id = manager.create_stack(
            "test-stack".to_string(),
            None, // Use default branch
            Some("Test stack description".to_string())
        ).unwrap();

        // Verify stack was created
        assert_eq!(manager.stacks.len(), 1);
        let stack = manager.get_stack(&stack_id).unwrap();
        assert_eq!(stack.name, "test-stack");
        // Should use the default branch (which gets set from the Git repo)
        assert!(!stack.base_branch.is_empty());
        assert!(stack.is_active);

        // Verify it's the active stack
        let active = manager.get_active_stack().unwrap();
        assert_eq!(active.id, stack_id);

        // Test get by name
        let found = manager.get_stack_by_name("test-stack").unwrap();
        assert_eq!(found.id, stack_id);
    }

    #[test]
    fn test_stack_persistence() {
        let (_temp_dir, repo_path) = create_test_repo();
        
        let stack_id = {
            let mut manager = StackManager::new(&repo_path).unwrap();
            manager.create_stack("persistent-stack".to_string(), None, None).unwrap()
        };

        // Create new manager and verify data was loaded
        let manager = StackManager::new(&repo_path).unwrap();
        assert_eq!(manager.stacks.len(), 1);
        let stack = manager.get_stack(&stack_id).unwrap();
        assert_eq!(stack.name, "persistent-stack");
    }

    #[test]
    fn test_multiple_stacks() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut manager = StackManager::new(&repo_path).unwrap();

        let stack1_id = manager.create_stack("stack-1".to_string(), None, None).unwrap();
        let stack2_id = manager.create_stack("stack-2".to_string(), None, None).unwrap();

        assert_eq!(manager.stacks.len(), 2);
        
        // First stack should still be active
        assert!(manager.get_stack(&stack1_id).unwrap().is_active);
        assert!(!manager.get_stack(&stack2_id).unwrap().is_active);

        // Change active stack
        manager.set_active_stack(Some(stack2_id)).unwrap();
        assert!(!manager.get_stack(&stack1_id).unwrap().is_active);
        assert!(manager.get_stack(&stack2_id).unwrap().is_active);
    }

    #[test]
    fn test_delete_stack() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut manager = StackManager::new(&repo_path).unwrap();

        let stack_id = manager.create_stack("to-delete".to_string(), None, None).unwrap();
        assert_eq!(manager.stacks.len(), 1);

        let deleted = manager.delete_stack(&stack_id).unwrap();
        assert_eq!(deleted.name, "to-delete");
        assert_eq!(manager.stacks.len(), 0);
        assert!(manager.get_active_stack().is_none());
    }

    #[test]
    fn test_validation() {
        let (_temp_dir, repo_path) = create_test_repo();
        let mut manager = StackManager::new(&repo_path).unwrap();

        manager.create_stack("valid-stack".to_string(), None, None).unwrap();
        
        // Should pass validation
        assert!(manager.validate_all().is_ok());
    }
} 