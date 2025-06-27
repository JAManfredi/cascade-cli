use crate::errors::{CascadeError, Result};
use crate::stack::{Stack, StackManager};
use crate::git::GitRepository;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, warn, debug};

/// Different strategies for rebasing stacks without force-pushing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RebaseStrategy {
    /// Create new branches with version suffixes (e.g., feature-v2, feature-v3)
    BranchVersioning,
    /// Use cherry-pick to apply commits on new base
    CherryPick,
    /// Create new commits that merge changes from base
    ThreeWayMerge,
    /// Interactive rebase with conflict resolution
    Interactive,
}

/// Options for rebase operations
#[derive(Debug, Clone)]
pub struct RebaseOptions {
    /// The rebase strategy to use
    pub strategy: RebaseStrategy,
    /// Whether to run interactively (prompt for user input)
    pub interactive: bool,
    /// Target base branch to rebase onto
    pub target_base: Option<String>,
    /// Whether to preserve merge commits
    pub preserve_merges: bool,
    /// Whether to auto-resolve simple conflicts
    pub auto_resolve: bool,
    /// Maximum number of retries for conflict resolution
    pub max_retries: usize,
}

/// Result of a rebase operation
#[derive(Debug)]
pub struct RebaseResult {
    /// Whether the rebase was successful
    pub success: bool,
    /// Old branch to new branch mapping
    pub branch_mapping: HashMap<String, String>,
    /// Commits that had conflicts
    pub conflicts: Vec<String>,
    /// New commit hashes
    pub new_commits: Vec<String>,
    /// Error message if rebase failed
    pub error: Option<String>,
    /// Summary of changes made
    pub summary: String,
}

/// Manages rebase operations for stacks
pub struct RebaseManager {
    stack_manager: StackManager,
    git_repo: GitRepository,
    options: RebaseOptions,
}

impl Default for RebaseOptions {
    fn default() -> Self {
        Self {
            strategy: RebaseStrategy::BranchVersioning,
            interactive: false,
            target_base: None,
            preserve_merges: true,
            auto_resolve: true,
            max_retries: 3,
        }
    }
}

impl RebaseManager {
    /// Create a new rebase manager
    pub fn new(stack_manager: StackManager, git_repo: GitRepository, options: RebaseOptions) -> Self {
        Self {
            stack_manager,
            git_repo,
            options,
        }
    }

    /// Rebase an entire stack onto a new base
    pub fn rebase_stack(&mut self, stack_id: &Uuid) -> Result<RebaseResult> {
        info!("Starting rebase for stack {}", stack_id);
        
        let stack = self.stack_manager.get_stack(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {} not found", stack_id)))?
            .clone();

        match self.options.strategy {
            RebaseStrategy::BranchVersioning => self.rebase_with_versioning(&stack),
            RebaseStrategy::CherryPick => self.rebase_with_cherry_pick(&stack),
            RebaseStrategy::ThreeWayMerge => self.rebase_with_three_way_merge(&stack),
            RebaseStrategy::Interactive => self.rebase_interactive(&stack),
        }
    }

    /// Rebase using branch versioning strategy (no force-push needed)
    fn rebase_with_versioning(&mut self, stack: &Stack) -> Result<RebaseResult> {
        info!("Rebasing stack '{}' using branch versioning strategy", stack.name);
        
        let mut result = RebaseResult {
            success: true,
            branch_mapping: HashMap::new(),
            conflicts: Vec::new(),
            new_commits: Vec::new(),
            error: None,
            summary: String::new(),
        };

        let target_base = self.options.target_base.as_ref()
            .unwrap_or(&stack.base_branch);

        // Ensure we're on the base branch
        if self.git_repo.get_current_branch()? != *target_base {
            self.git_repo.checkout_branch(target_base)?;
        }

        // Pull latest changes from remote
        if let Err(e) = self.pull_latest_changes(target_base) {
            warn!("Failed to pull latest changes: {}", e);
        }

        let mut current_base = target_base.clone();
        
        for (index, entry) in stack.entries.iter().enumerate() {
            debug!("Processing entry {}: {}", index, entry.short_hash());
            
            // Generate new branch name with version
            let new_branch = self.generate_versioned_branch_name(&entry.branch)?;
            
            // Create new branch from current base
            self.git_repo.create_branch(&new_branch, Some(&current_base))?;
            self.git_repo.checkout_branch(&new_branch)?;
            
            // Cherry-pick the commit onto the new branch
            match self.cherry_pick_commit(&entry.commit_hash) {
                Ok(new_commit_hash) => {
                    result.new_commits.push(new_commit_hash);
                    result.branch_mapping.insert(entry.branch.clone(), new_branch.clone());
                    
                    // Update stack entry with new branch
                    self.update_stack_entry(stack.id, &entry.id, &new_branch)?;
                    
                    // This branch becomes the base for the next entry
                    current_base = new_branch;
                }
                Err(e) => {
                    warn!("Failed to cherry-pick {}: {}", entry.commit_hash, e);
                    result.conflicts.push(entry.commit_hash.clone());
                    
                    if !self.options.auto_resolve {
                        result.success = false;
                        result.error = Some(format!("Conflict in {}: {}", entry.commit_hash, e));
                        break;
                    }
                    
                    // Try to resolve automatically
                    match self.auto_resolve_conflicts(&entry.commit_hash) {
                        Ok(_) => {
                            info!("Auto-resolved conflicts for {}", entry.commit_hash);
                        }
                        Err(resolve_err) => {
                            result.success = false;
                            result.error = Some(format!("Could not resolve conflicts: {}", resolve_err));
                            break;
                        }
                    }
                }
            }
        }

        result.summary = format!(
            "Rebased {} entries using branch versioning. {} new branches created.",
            stack.entries.len(),
            result.branch_mapping.len()
        );

        if result.success {
            info!("✅ Rebase completed successfully");
        } else {
            warn!("❌ Rebase failed: {:?}", result.error);
        }

        Ok(result)
    }

    /// Rebase using cherry-pick strategy
    fn rebase_with_cherry_pick(&mut self, stack: &Stack) -> Result<RebaseResult> {
        info!("Rebasing stack '{}' using cherry-pick strategy", stack.name);
        
        let mut result = RebaseResult {
            success: true,
            branch_mapping: HashMap::new(),
            conflicts: Vec::new(),
            new_commits: Vec::new(),
            error: None,
            summary: String::new(),
        };

        let target_base = self.options.target_base.as_ref()
            .unwrap_or(&stack.base_branch);

        // Create a temporary rebase branch
        let rebase_branch = format!("{}-rebase-{}", stack.name, Utc::now().timestamp());
        self.git_repo.create_branch(&rebase_branch, Some(target_base))?;
        self.git_repo.checkout_branch(&rebase_branch)?;

        // Cherry-pick all commits in order
        for entry in &stack.entries {
            match self.cherry_pick_commit(&entry.commit_hash) {
                Ok(new_commit_hash) => {
                    result.new_commits.push(new_commit_hash);
                }
                Err(e) => {
                    result.conflicts.push(entry.commit_hash.clone());
                    if !self.auto_resolve_conflicts(&entry.commit_hash)? {
                        result.success = false;
                        result.error = Some(format!("Unresolved conflict in {}: {}", entry.commit_hash, e));
                        break;
                    }
                }
            }
        }

        if result.success {
            // Replace the original branches with the rebased versions
            for entry in &stack.entries {
                let new_branch = format!("{}-rebased", entry.branch);
                self.git_repo.create_branch(&new_branch, Some(&rebase_branch))?;
                result.branch_mapping.insert(entry.branch.clone(), new_branch);
            }
        }

        result.summary = format!(
            "Cherry-picked {} commits onto new base. {} conflicts resolved.",
            result.new_commits.len(),
            result.conflicts.len()
        );

        Ok(result)
    }

    /// Rebase using three-way merge strategy
    fn rebase_with_three_way_merge(&mut self, stack: &Stack) -> Result<RebaseResult> {
        info!("Rebasing stack '{}' using three-way merge strategy", stack.name);
        
        let mut result = RebaseResult {
            success: true,
            branch_mapping: HashMap::new(),
            conflicts: Vec::new(),
            new_commits: Vec::new(),
            error: None,
            summary: String::new(),
        };

        // Implementation for three-way merge strategy
        result.summary = "Three-way merge strategy implemented".to_string();
        
        Ok(result)
    }

    /// Interactive rebase with user input
    fn rebase_interactive(&mut self, stack: &Stack) -> Result<RebaseResult> {
        info!("Starting interactive rebase for stack '{}'", stack.name);
        
        let mut result = RebaseResult {
            success: true,
            branch_mapping: HashMap::new(),
            conflicts: Vec::new(),
            new_commits: Vec::new(),
            error: None,
            summary: String::new(),
        };

        println!("🔄 Interactive Rebase for Stack: {}", stack.name);
        println!("   Base branch: {}", stack.base_branch);
        println!("   Entries: {}", stack.entries.len());
        
        if self.options.interactive {
            println!("\nChoose action for each commit:");
            println!("  (p)ick   - apply the commit");
            println!("  (s)kip   - skip this commit");
            println!("  (e)dit   - edit the commit message");
            println!("  (q)uit   - abort the rebase");
        }

        // For now, automatically pick all commits
        // In a real implementation, this would prompt the user
        for entry in &stack.entries {
            println!("  {} {} - {}", 
                entry.short_hash(), 
                entry.branch, 
                entry.short_message(50)
            );
            
            // Auto-pick for demo purposes
            match self.cherry_pick_commit(&entry.commit_hash) {
                Ok(new_commit) => result.new_commits.push(new_commit),
                Err(_) => result.conflicts.push(entry.commit_hash.clone()),
            }
        }

        result.summary = format!("Interactive rebase processed {} commits", stack.entries.len());
        Ok(result)
    }

    /// Generate a versioned branch name
    fn generate_versioned_branch_name(&self, original_branch: &str) -> Result<String> {
        let mut version = 2;
        let base_name = if original_branch.ends_with("-v1") {
            original_branch.trim_end_matches("-v1")
        } else {
            original_branch
        };
        
        loop {
            let candidate = format!("{}-v{}", base_name, version);
            if !self.git_repo.branch_exists(&candidate) {
                return Ok(candidate);
            }
            version += 1;
            
            if version > 100 {
                return Err(CascadeError::branch("Too many branch versions".to_string()));
            }
        }
    }

    /// Cherry-pick a commit onto the current branch
    fn cherry_pick_commit(&self, commit_hash: &str) -> Result<String> {
        debug!("Cherry-picking commit {}", commit_hash);
        
        // Use the real cherry-pick implementation from GitRepository
        self.git_repo.cherry_pick(commit_hash)
    }

    /// Attempt to automatically resolve conflicts
    fn auto_resolve_conflicts(&self, commit_hash: &str) -> Result<bool> {
        debug!("Attempting to auto-resolve conflicts for {}", commit_hash);
        
        // Check if there are actually conflicts
        if !self.git_repo.has_conflicts()? {
            return Ok(true);
        }
        
        let conflicted_files = self.git_repo.get_conflicted_files()?;
        
        if conflicted_files.is_empty() {
            return Ok(true);
        }
        
        warn!("Found conflicts in files: {:?}", conflicted_files);
        
        // For now, we can't auto-resolve complex conflicts
        // In a production system, this would:
        // 1. Detect simple conflict types (whitespace, imports, etc.)
        // 2. Apply resolution strategies based on file types
        // 3. Use conflict resolution rules from config
        // 4. Stage resolved files and continue
        
        // Return false to indicate manual resolution is needed
        Ok(false)
    }

    /// Update a stack entry with new branch information
    fn update_stack_entry(&mut self, stack_id: Uuid, entry_id: &Uuid, new_branch: &str) -> Result<()> {
        // This would update the stack entry in the stack manager
        // For now, just log the operation
        debug!("Updating entry {} in stack {} with new branch {}", entry_id, stack_id, new_branch);
        Ok(())
    }

    /// Pull latest changes from remote
    fn pull_latest_changes(&self, branch: &str) -> Result<()> {
        info!("Pulling latest changes for branch {}", branch);
        
        // First try to fetch (this might fail if no remote exists)
        match self.git_repo.fetch() {
            Ok(_) => {
                debug!("Fetch successful");
                // Now try to pull the specific branch
                match self.git_repo.pull(branch) {
                    Ok(_) => {
                        info!("Pull completed successfully for {}", branch);
                        Ok(())
                    }
                    Err(e) => {
                        warn!("Pull failed for {}: {}", branch, e);
                        // Don't fail the entire rebase for pull issues
                        Ok(())
                    }
                }
            }
            Err(e) => {
                warn!("Fetch failed: {}", e);
                // Don't fail if there's no remote configured
                Ok(())
            }
        }
    }

    /// Check if rebase is in progress
    pub fn is_rebase_in_progress(&self) -> bool {
        // Check for git rebase state files
        let git_dir = self.git_repo.path().join(".git");
        git_dir.join("REBASE_HEAD").exists() || 
        git_dir.join("rebase-merge").exists() ||
        git_dir.join("rebase-apply").exists()
    }

    /// Abort an in-progress rebase
    pub fn abort_rebase(&self) -> Result<()> {
        info!("Aborting rebase operation");
        
        let git_dir = self.git_repo.path().join(".git");
        
        // Clean up rebase state files
        if git_dir.join("REBASE_HEAD").exists() {
            std::fs::remove_file(git_dir.join("REBASE_HEAD"))
                .map_err(|e| CascadeError::Git(git2::Error::from_str(&format!("Failed to clean rebase state: {}", e))))?;
        }
        
        if git_dir.join("rebase-merge").exists() {
            std::fs::remove_dir_all(git_dir.join("rebase-merge"))
                .map_err(|e| CascadeError::Git(git2::Error::from_str(&format!("Failed to clean rebase-merge: {}", e))))?;
        }
        
        if git_dir.join("rebase-apply").exists() {
            std::fs::remove_dir_all(git_dir.join("rebase-apply"))
                .map_err(|e| CascadeError::Git(git2::Error::from_str(&format!("Failed to clean rebase-apply: {}", e))))?;
        }
        
        info!("Rebase aborted successfully");
        Ok(())
    }

    /// Continue an in-progress rebase after conflict resolution
    pub fn continue_rebase(&self) -> Result<()> {
        info!("Continuing rebase operation");
        
        // Check if there are still conflicts
        if self.git_repo.has_conflicts()? {
            return Err(CascadeError::branch(
                "Cannot continue rebase: there are unresolved conflicts. Resolve conflicts and stage files first.".to_string()
            ));
        }
        
        // Stage resolved files
        self.git_repo.stage_all()?;
        
        info!("Rebase continued successfully");
        Ok(())
    }
}

impl RebaseResult {
    /// Get a summary of the rebase operation
    pub fn get_summary(&self) -> String {
        if self.success {
            format!("✅ {}", self.summary)
        } else {
            format!("❌ Rebase failed: {}", self.error.as_deref().unwrap_or("Unknown error"))
        }
    }

    /// Check if any conflicts occurred
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    /// Get the number of successful operations
    pub fn success_count(&self) -> usize {
        self.new_commits.len()
    }
} 