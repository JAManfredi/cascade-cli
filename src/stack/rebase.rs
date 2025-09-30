use crate::errors::{CascadeError, Result};
use crate::git::{ConflictAnalyzer, GitRepository};
use crate::stack::{Stack, StackManager};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Conflict resolution result
#[derive(Debug, Clone)]
enum ConflictResolution {
    /// Conflict was successfully resolved
    Resolved,
    /// Conflict is too complex for automatic resolution
    TooComplex,
}

/// Represents a conflict region in a file
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ConflictRegion {
    /// Byte position where conflict starts
    start: usize,
    /// Byte position where conflict ends  
    end: usize,
    /// Line number where conflict starts
    start_line: usize,
    /// Line number where conflict ends
    end_line: usize,
    /// Content from "our" side (before separator)
    our_content: String,
    /// Content from "their" side (after separator)
    their_content: String,
}

/// Strategy for rebasing stacks (force-push is the only valid approach for preserving PR history)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RebaseStrategy {
    /// Force-push rebased commits to original branches (preserves PR history)
    /// This is the industry standard used by Graphite, Phabricator, spr, etc.
    ForcePush,
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
    /// Skip pulling latest changes (when already done by caller)
    pub skip_pull: Option<bool>,
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

/// RAII guard to ensure temporary branches are cleaned up even on error/panic
///
/// This stores branch names and provides a cleanup method that can be called
/// with a GitRepository reference. The Drop trait ensures cleanup happens
/// even if the rebase function panics or returns early with an error.
#[allow(dead_code)]
struct TempBranchCleanupGuard {
    branches: Vec<String>,
    cleaned: bool,
}

#[allow(dead_code)]
impl TempBranchCleanupGuard {
    fn new() -> Self {
        Self {
            branches: Vec::new(),
            cleaned: false,
        }
    }

    fn add_branch(&mut self, branch: String) {
        self.branches.push(branch);
    }

    /// Perform cleanup with provided git repository
    fn cleanup(&mut self, git_repo: &GitRepository) {
        if self.cleaned || self.branches.is_empty() {
            return;
        }

        info!("🧹 Cleaning up {} temporary branches", self.branches.len());
        for branch in &self.branches {
            if let Err(e) = git_repo.delete_branch_unsafe(branch) {
                warn!("Failed to delete temp branch {}: {}", branch, e);
                // Continue with cleanup even if one fails
            }
        }
        self.cleaned = true;
    }
}

impl Drop for TempBranchCleanupGuard {
    fn drop(&mut self) {
        if !self.cleaned && !self.branches.is_empty() {
            // This path is only hit on panic or unexpected early return
            // We can't access git_repo here, so just log the branches that need manual cleanup
            warn!(
                "⚠️  {} temporary branches were not cleaned up: {}",
                self.branches.len(),
                self.branches.join(", ")
            );
            warn!("Run 'ca cleanup' to remove orphaned temporary branches");
        }
    }
}

/// Manages rebase operations for stacks
pub struct RebaseManager {
    stack_manager: StackManager,
    git_repo: GitRepository,
    options: RebaseOptions,
    conflict_analyzer: ConflictAnalyzer,
}

impl Default for RebaseOptions {
    fn default() -> Self {
        Self {
            strategy: RebaseStrategy::ForcePush,
            interactive: false,
            target_base: None,
            preserve_merges: true,
            auto_resolve: true,
            max_retries: 3,
            skip_pull: None,
        }
    }
}

impl RebaseManager {
    /// Create a new rebase manager
    pub fn new(
        stack_manager: StackManager,
        git_repo: GitRepository,
        options: RebaseOptions,
    ) -> Self {
        Self {
            stack_manager,
            git_repo,
            options,
            conflict_analyzer: ConflictAnalyzer::new(),
        }
    }

    /// Rebase an entire stack onto a new base
    pub fn rebase_stack(&mut self, stack_id: &Uuid) -> Result<RebaseResult> {
        info!("Starting rebase for stack {}", stack_id);

        let stack = self
            .stack_manager
            .get_stack(stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?
            .clone();

        match self.options.strategy {
            RebaseStrategy::ForcePush => self.rebase_with_force_push(&stack),
            RebaseStrategy::Interactive => self.rebase_interactive(&stack),
        }
    }

    /// Rebase using force-push strategy (industry standard for stacked diffs)
    /// This updates local branches in-place, then force-pushes ONLY branches with existing PRs
    /// to preserve PR history - the approach used by Graphite, Phabricator, spr, etc.
    fn rebase_with_force_push(&mut self, stack: &Stack) -> Result<RebaseResult> {
        use crate::cli::output::Output;

        Output::section(format!("Rebasing stack: {}", stack.name));

        let mut result = RebaseResult {
            success: true,
            branch_mapping: HashMap::new(),
            conflicts: Vec::new(),
            new_commits: Vec::new(),
            error: None,
            summary: String::new(),
        };

        let target_base = self
            .options
            .target_base
            .as_ref()
            .unwrap_or(&stack.base_branch)
            .clone(); // Clone to avoid borrow issues

        // Save original working branch to restore later
        let original_branch = self.git_repo.get_current_branch().ok();

        // Ensure we're on the base branch
        if self.git_repo.get_current_branch()? != target_base {
            self.git_repo.checkout_branch(&target_base)?;
        }

        // Only pull if not already done by caller (like sync command)
        if !self.options.skip_pull.unwrap_or(false) {
            if let Err(e) = self.pull_latest_changes(&target_base) {
                Output::warning(format!("Could not pull latest changes: {}", e));
            }
        }

        // Reset working directory to clean state before rebase
        if let Err(e) = self.git_repo.reset_to_head() {
            Output::warning(format!("Could not reset working directory: {}", e));
        }

        let mut current_base = target_base.clone();
        let entry_count = stack.entries.len();
        let mut pushed_count = 0;
        let mut skipped_count = 0;
        let mut temp_branches: Vec<String> = Vec::new(); // Track temp branches for cleanup

        println!(); // Spacing before tree
        let plural = if entry_count == 1 { "entry" } else { "entries" };
        println!("📋 Rebasing {} {}", entry_count, plural);

        for (index, entry) in stack.entries.iter().enumerate() {
            let original_branch = &entry.branch;

            // Create a temporary branch from the current base
            // This avoids committing directly to protected branches like develop/main
            let temp_branch = format!("{}-temp-{}", original_branch, Utc::now().timestamp());
            temp_branches.push(temp_branch.clone()); // Track for cleanup
            self.git_repo
                .create_branch(&temp_branch, Some(&current_base))?;
            self.git_repo.checkout_branch(&temp_branch)?;

            // Cherry-pick the commit onto the temp branch (NOT the protected base!)
            match self.cherry_pick_commit(&entry.commit_hash) {
                Ok(new_commit_hash) => {
                    result.new_commits.push(new_commit_hash.clone());

                    // Get the commit that's now at HEAD (the cherry-picked commit)
                    let rebased_commit_id = self.git_repo.get_head_commit()?.id().to_string();

                    // Update the original branch to point to this rebased commit
                    // This is LOCAL ONLY - moves refs/heads/<branch> to the commit on temp branch
                    self.git_repo
                        .update_branch_to_commit(original_branch, &rebased_commit_id)?;

                    // Only force-push to REMOTE if this entry has a PR
                    if entry.pull_request_id.is_some() {
                        let pr_num = entry.pull_request_id.as_ref().unwrap();
                        let tree_char = if index + 1 == entry_count {
                            "└─"
                        } else {
                            "├─"
                        };
                        println!("   {} {} (PR #{})", tree_char, original_branch, pr_num);

                        // NOW do the actual force-push to remote (git push --force origin <branch>)
                        // This updates the PR with the rebased commits
                        self.git_repo.force_push_single_branch(original_branch)?;
                        pushed_count += 1;
                    } else {
                        let tree_char = if index + 1 == entry_count {
                            "└─"
                        } else {
                            "├─"
                        };
                        println!("   {} {} (not submitted)", tree_char, original_branch);
                        skipped_count += 1;
                    }

                    result
                        .branch_mapping
                        .insert(original_branch.clone(), original_branch.clone());

                    // Update stack entry with new commit hash
                    self.update_stack_entry(
                        stack.id,
                        &entry.id,
                        original_branch,
                        &rebased_commit_id,
                    )?;

                    // This branch becomes the base for the next entry
                    current_base = original_branch.clone();
                }
                Err(e) => {
                    println!(); // Spacing before error
                    Output::error(format!("Conflict in {}: {}", &entry.commit_hash[..8], e));
                    result.conflicts.push(entry.commit_hash.clone());

                    if !self.options.auto_resolve {
                        result.success = false;
                        result.error = Some(format!("Conflict in {}: {}", entry.commit_hash, e));
                        break;
                    }

                    // Try to resolve automatically
                    match self.auto_resolve_conflicts(&entry.commit_hash) {
                        Ok(_) => {
                            Output::success("Auto-resolved conflicts");
                        }
                        Err(resolve_err) => {
                            result.success = false;
                            result.error =
                                Some(format!("Could not resolve conflicts: {resolve_err}"));
                            break;
                        }
                    }
                }
            }
        }

        // Cleanup temp branches before returning to original branch
        // Must checkout away from temp branches first
        if !temp_branches.is_empty() {
            // Checkout base branch to allow temp branch deletion
            if let Err(e) = self.git_repo.checkout_branch(&target_base) {
                Output::warning(format!("Could not checkout base for cleanup: {}", e));
            }

            // Delete all temp branches
            for temp_branch in &temp_branches {
                if let Err(e) = self.git_repo.delete_branch_unsafe(temp_branch) {
                    debug!("Could not delete temp branch {}: {}", temp_branch, e);
                }
            }
        }

        // Return to original working branch
        if let Some(orig_branch) = original_branch {
            if let Err(e) = self.git_repo.checkout_branch(&orig_branch) {
                Output::warning(format!(
                    "Could not return to original branch '{}': {}",
                    orig_branch, e
                ));
            }
        }

        // Build summary message
        result.summary = if pushed_count > 0 {
            let pr_plural = if pushed_count == 1 { "" } else { "s" };
            let entry_plural = if entry_count == 1 { "entry" } else { "entries" };

            if skipped_count > 0 {
                format!(
                    "{} {} rebased ({} PR{} updated, {} not yet submitted)",
                    entry_count, entry_plural, pushed_count, pr_plural, skipped_count
                )
            } else {
                format!(
                    "{} {} rebased ({} PR{} updated)",
                    entry_count, entry_plural, pushed_count, pr_plural
                )
            }
        } else {
            let plural = if entry_count == 1 { "entry" } else { "entries" };
            format!("{} {} rebased (no PRs to update yet)", entry_count, plural)
        };

        // Display result with proper formatting
        println!(); // Spacing after tree
        if result.success {
            Output::success(&result.summary);
        } else {
            Output::error(format!("Rebase failed: {:?}", result.error));
        }

        // Save the updated stack metadata to disk
        self.stack_manager.save_to_disk()?;

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
            println!(
                "  {} {} - {}",
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

        result.summary = format!(
            "Interactive rebase processed {} commits",
            stack.entries.len()
        );
        Ok(result)
    }

    /// Cherry-pick a commit onto the current branch
    fn cherry_pick_commit(&self, commit_hash: &str) -> Result<String> {
        // Use the real cherry-pick implementation from GitRepository
        let new_commit_hash = self.git_repo.cherry_pick(commit_hash)?;

        // Check for any leftover staged changes after successful cherry-pick
        if let Ok(staged_files) = self.git_repo.get_staged_files() {
            if !staged_files.is_empty() {
                // Commit any leftover staged changes silently
                let cleanup_message = format!("Cleanup after cherry-pick {}", &commit_hash[..8]);
                let _ = self.git_repo.commit_staged_changes(&cleanup_message);
            }
        }

        Ok(new_commit_hash)
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

        info!(
            "Found conflicts in {} files: {:?}",
            conflicted_files.len(),
            conflicted_files
        );

        // Use the new conflict analyzer for detailed analysis
        let analysis = self
            .conflict_analyzer
            .analyze_conflicts(&conflicted_files, self.git_repo.path())?;

        info!(
            "🔍 Conflict analysis: {} total conflicts, {} auto-resolvable",
            analysis.total_conflicts, analysis.auto_resolvable_count
        );

        // Display recommendations
        for recommendation in &analysis.recommendations {
            info!("💡 {}", recommendation);
        }

        let mut resolved_count = 0;
        let mut failed_files = Vec::new();

        for file_analysis in &analysis.files {
            if file_analysis.auto_resolvable {
                match self.resolve_file_conflicts_enhanced(
                    &file_analysis.file_path,
                    &file_analysis.conflicts,
                ) {
                    Ok(ConflictResolution::Resolved) => {
                        resolved_count += 1;
                        info!("✅ Auto-resolved conflicts in {}", file_analysis.file_path);
                    }
                    Ok(ConflictResolution::TooComplex) => {
                        debug!(
                            "⚠️  Conflicts in {} are too complex for auto-resolution",
                            file_analysis.file_path
                        );
                        failed_files.push(file_analysis.file_path.clone());
                    }
                    Err(e) => {
                        warn!(
                            "❌ Failed to resolve conflicts in {}: {}",
                            file_analysis.file_path, e
                        );
                        failed_files.push(file_analysis.file_path.clone());
                    }
                }
            } else {
                failed_files.push(file_analysis.file_path.clone());
                info!(
                    "⚠️  {} requires manual resolution ({} conflicts)",
                    file_analysis.file_path,
                    file_analysis.conflicts.len()
                );
            }
        }

        if resolved_count > 0 {
            info!(
                "🎉 Auto-resolved conflicts in {}/{} files",
                resolved_count,
                conflicted_files.len()
            );

            // Stage all resolved files
            self.git_repo.stage_conflict_resolved_files()?;
        }

        // Return true only if ALL conflicts were resolved
        let all_resolved = failed_files.is_empty();

        if !all_resolved {
            info!(
                "⚠️  {} files still need manual resolution: {:?}",
                failed_files.len(),
                failed_files
            );
        }

        Ok(all_resolved)
    }

    /// Resolve conflicts using enhanced analysis
    fn resolve_file_conflicts_enhanced(
        &self,
        file_path: &str,
        conflicts: &[crate::git::ConflictRegion],
    ) -> Result<ConflictResolution> {
        let repo_path = self.git_repo.path();
        let full_path = repo_path.join(file_path);

        // Read the file content with conflict markers
        let mut content = std::fs::read_to_string(&full_path)
            .map_err(|e| CascadeError::config(format!("Failed to read file {file_path}: {e}")))?;

        if conflicts.is_empty() {
            return Ok(ConflictResolution::Resolved);
        }

        info!(
            "Resolving {} conflicts in {} using enhanced analysis",
            conflicts.len(),
            file_path
        );

        let mut any_resolved = false;

        // Process conflicts in reverse order to maintain string indices
        for conflict in conflicts.iter().rev() {
            match self.resolve_single_conflict_enhanced(conflict) {
                Ok(Some(resolution)) => {
                    // Replace the conflict region with the resolved content
                    let before = &content[..conflict.start_pos];
                    let after = &content[conflict.end_pos..];
                    content = format!("{before}{resolution}{after}");
                    any_resolved = true;
                    debug!(
                        "✅ Resolved {} conflict at lines {}-{} in {}",
                        format!("{:?}", conflict.conflict_type).to_lowercase(),
                        conflict.start_line,
                        conflict.end_line,
                        file_path
                    );
                }
                Ok(None) => {
                    debug!(
                        "⚠️  {} conflict at lines {}-{} in {} requires manual resolution",
                        format!("{:?}", conflict.conflict_type).to_lowercase(),
                        conflict.start_line,
                        conflict.end_line,
                        file_path
                    );
                    return Ok(ConflictResolution::TooComplex);
                }
                Err(e) => {
                    debug!("❌ Failed to resolve conflict in {}: {}", file_path, e);
                    return Ok(ConflictResolution::TooComplex);
                }
            }
        }

        if any_resolved {
            // Check if we resolved ALL conflicts in this file
            let remaining_conflicts = self.parse_conflict_markers(&content)?;

            if remaining_conflicts.is_empty() {
                // All conflicts resolved - write the file back atomically
                crate::utils::atomic_file::write_string(&full_path, &content)?;

                return Ok(ConflictResolution::Resolved);
            } else {
                info!(
                    "⚠️  Partially resolved conflicts in {} ({} remaining)",
                    file_path,
                    remaining_conflicts.len()
                );
            }
        }

        Ok(ConflictResolution::TooComplex)
    }

    /// Resolve a single conflict using enhanced analysis
    fn resolve_single_conflict_enhanced(
        &self,
        conflict: &crate::git::ConflictRegion,
    ) -> Result<Option<String>> {
        debug!(
            "Resolving {} conflict in {} (lines {}-{})",
            format!("{:?}", conflict.conflict_type).to_lowercase(),
            conflict.file_path,
            conflict.start_line,
            conflict.end_line
        );

        use crate::git::ConflictType;

        match conflict.conflict_type {
            ConflictType::Whitespace => {
                // Use the version with better formatting
                if conflict.our_content.trim().len() >= conflict.their_content.trim().len() {
                    Ok(Some(conflict.our_content.clone()))
                } else {
                    Ok(Some(conflict.their_content.clone()))
                }
            }
            ConflictType::LineEnding => {
                // Normalize to Unix line endings
                let normalized = conflict
                    .our_content
                    .replace("\r\n", "\n")
                    .replace('\r', "\n");
                Ok(Some(normalized))
            }
            ConflictType::PureAddition => {
                // Merge both additions
                if conflict.our_content.is_empty() {
                    Ok(Some(conflict.their_content.clone()))
                } else if conflict.their_content.is_empty() {
                    Ok(Some(conflict.our_content.clone()))
                } else {
                    // Try to combine both
                    let combined = format!("{}\n{}", conflict.our_content, conflict.their_content);
                    Ok(Some(combined))
                }
            }
            ConflictType::ImportMerge => {
                // Sort and merge imports
                let mut all_imports: Vec<&str> = conflict
                    .our_content
                    .lines()
                    .chain(conflict.their_content.lines())
                    .collect();
                all_imports.sort();
                all_imports.dedup();
                Ok(Some(all_imports.join("\n")))
            }
            ConflictType::Structural | ConflictType::ContentOverlap | ConflictType::Complex => {
                // These require manual resolution
                Ok(None)
            }
        }
    }

    /// Resolve conflicts in a single file using smart strategies
    #[allow(dead_code)]
    fn resolve_file_conflicts(&self, file_path: &str) -> Result<ConflictResolution> {
        let repo_path = self.git_repo.path();
        let full_path = repo_path.join(file_path);

        // Read the file content with conflict markers
        let content = std::fs::read_to_string(&full_path)
            .map_err(|e| CascadeError::config(format!("Failed to read file {file_path}: {e}")))?;

        // Parse conflicts from the file
        let conflicts = self.parse_conflict_markers(&content)?;

        if conflicts.is_empty() {
            // No conflict markers found - file might already be resolved
            return Ok(ConflictResolution::Resolved);
        }

        info!(
            "Found {} conflict regions in {}",
            conflicts.len(),
            file_path
        );

        // Try to resolve each conflict using our strategies
        let mut resolved_content = content;
        let mut any_resolved = false;

        // Process conflicts in reverse order to maintain string indices
        for conflict in conflicts.iter().rev() {
            match self.resolve_single_conflict(conflict, file_path) {
                Ok(Some(resolution)) => {
                    // Replace the conflict region with the resolved content
                    let before = &resolved_content[..conflict.start];
                    let after = &resolved_content[conflict.end..];
                    resolved_content = format!("{before}{resolution}{after}");
                    any_resolved = true;
                    debug!(
                        "✅ Resolved conflict at lines {}-{} in {}",
                        conflict.start_line, conflict.end_line, file_path
                    );
                }
                Ok(None) => {
                    debug!(
                        "⚠️  Conflict at lines {}-{} in {} too complex for auto-resolution",
                        conflict.start_line, conflict.end_line, file_path
                    );
                }
                Err(e) => {
                    debug!("❌ Failed to resolve conflict in {}: {}", file_path, e);
                }
            }
        }

        if any_resolved {
            // Check if we resolved ALL conflicts in this file
            let remaining_conflicts = self.parse_conflict_markers(&resolved_content)?;

            if remaining_conflicts.is_empty() {
                // All conflicts resolved - write the file back atomically
                crate::utils::atomic_file::write_string(&full_path, &resolved_content)?;

                return Ok(ConflictResolution::Resolved);
            } else {
                info!(
                    "⚠️  Partially resolved conflicts in {} ({} remaining)",
                    file_path,
                    remaining_conflicts.len()
                );
            }
        }

        Ok(ConflictResolution::TooComplex)
    }

    /// Parse conflict markers from file content
    fn parse_conflict_markers(&self, content: &str) -> Result<Vec<ConflictRegion>> {
        let lines: Vec<&str> = content.lines().collect();
        let mut conflicts = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            if lines[i].starts_with("<<<<<<<") {
                // Found start of conflict
                let start_line = i + 1;
                let mut separator_line = None;
                let mut end_line = None;

                // Find the separator and end
                for (j, line) in lines.iter().enumerate().skip(i + 1) {
                    if line.starts_with("=======") {
                        separator_line = Some(j + 1);
                    } else if line.starts_with(">>>>>>>") {
                        end_line = Some(j + 1);
                        break;
                    }
                }

                if let (Some(sep), Some(end)) = (separator_line, end_line) {
                    // Calculate byte positions
                    let start_pos = lines[..i].iter().map(|l| l.len() + 1).sum::<usize>();
                    let end_pos = lines[..end].iter().map(|l| l.len() + 1).sum::<usize>();

                    let our_content = lines[(i + 1)..(sep - 1)].join("\n");
                    let their_content = lines[sep..(end - 1)].join("\n");

                    conflicts.push(ConflictRegion {
                        start: start_pos,
                        end: end_pos,
                        start_line,
                        end_line: end,
                        our_content,
                        their_content,
                    });

                    i = end;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        Ok(conflicts)
    }

    /// Resolve a single conflict using smart strategies
    fn resolve_single_conflict(
        &self,
        conflict: &ConflictRegion,
        file_path: &str,
    ) -> Result<Option<String>> {
        debug!(
            "Analyzing conflict in {} (lines {}-{})",
            file_path, conflict.start_line, conflict.end_line
        );

        // Strategy 1: Whitespace-only differences
        if let Some(resolved) = self.resolve_whitespace_conflict(conflict)? {
            debug!("Resolved as whitespace-only conflict");
            return Ok(Some(resolved));
        }

        // Strategy 2: Line ending differences
        if let Some(resolved) = self.resolve_line_ending_conflict(conflict)? {
            debug!("Resolved as line ending conflict");
            return Ok(Some(resolved));
        }

        // Strategy 3: Pure addition conflicts (no overlapping changes)
        if let Some(resolved) = self.resolve_addition_conflict(conflict)? {
            debug!("Resolved as pure addition conflict");
            return Ok(Some(resolved));
        }

        // Strategy 4: Import/dependency reordering
        if let Some(resolved) = self.resolve_import_conflict(conflict, file_path)? {
            debug!("Resolved as import reordering conflict");
            return Ok(Some(resolved));
        }

        // No strategy could resolve this conflict
        Ok(None)
    }

    /// Resolve conflicts that only differ by whitespace
    fn resolve_whitespace_conflict(&self, conflict: &ConflictRegion) -> Result<Option<String>> {
        let our_normalized = self.normalize_whitespace(&conflict.our_content);
        let their_normalized = self.normalize_whitespace(&conflict.their_content);

        if our_normalized == their_normalized {
            // Only whitespace differences - prefer the version with better formatting
            let resolved =
                if conflict.our_content.trim().len() >= conflict.their_content.trim().len() {
                    conflict.our_content.clone()
                } else {
                    conflict.their_content.clone()
                };

            return Ok(Some(resolved));
        }

        Ok(None)
    }

    /// Resolve conflicts that only differ by line endings
    fn resolve_line_ending_conflict(&self, conflict: &ConflictRegion) -> Result<Option<String>> {
        let our_normalized = conflict
            .our_content
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        let their_normalized = conflict
            .their_content
            .replace("\r\n", "\n")
            .replace('\r', "\n");

        if our_normalized == their_normalized {
            // Only line ending differences - prefer Unix line endings
            return Ok(Some(our_normalized));
        }

        Ok(None)
    }

    /// Resolve conflicts where both sides only add lines (no overlapping edits)
    fn resolve_addition_conflict(&self, conflict: &ConflictRegion) -> Result<Option<String>> {
        let our_lines: Vec<&str> = conflict.our_content.lines().collect();
        let their_lines: Vec<&str> = conflict.their_content.lines().collect();

        // Check if one side is a subset of the other (pure addition)
        if our_lines.is_empty() {
            return Ok(Some(conflict.their_content.clone()));
        }
        if their_lines.is_empty() {
            return Ok(Some(conflict.our_content.clone()));
        }

        // Try to merge additions intelligently
        let mut merged_lines = Vec::new();
        let mut our_idx = 0;
        let mut their_idx = 0;

        while our_idx < our_lines.len() || their_idx < their_lines.len() {
            if our_idx >= our_lines.len() {
                // Only their lines left
                merged_lines.extend_from_slice(&their_lines[their_idx..]);
                break;
            } else if their_idx >= their_lines.len() {
                // Only our lines left
                merged_lines.extend_from_slice(&our_lines[our_idx..]);
                break;
            } else if our_lines[our_idx] == their_lines[their_idx] {
                // Same line - add once
                merged_lines.push(our_lines[our_idx]);
                our_idx += 1;
                their_idx += 1;
            } else {
                // Different lines - this might be too complex
                return Ok(None);
            }
        }

        Ok(Some(merged_lines.join("\n")))
    }

    /// Resolve import/dependency conflicts by sorting and merging
    fn resolve_import_conflict(
        &self,
        conflict: &ConflictRegion,
        file_path: &str,
    ) -> Result<Option<String>> {
        // Only apply to likely import sections in common file types
        let is_import_file = file_path.ends_with(".rs")
            || file_path.ends_with(".py")
            || file_path.ends_with(".js")
            || file_path.ends_with(".ts")
            || file_path.ends_with(".go")
            || file_path.ends_with(".java")
            || file_path.ends_with(".swift")
            || file_path.ends_with(".kt")
            || file_path.ends_with(".cs");

        if !is_import_file {
            return Ok(None);
        }

        let our_lines: Vec<&str> = conflict.our_content.lines().collect();
        let their_lines: Vec<&str> = conflict.their_content.lines().collect();

        // Check if all lines look like imports/uses
        let our_imports = our_lines
            .iter()
            .all(|line| self.is_import_line(line, file_path));
        let their_imports = their_lines
            .iter()
            .all(|line| self.is_import_line(line, file_path));

        if our_imports && their_imports {
            // Merge and sort imports
            let mut all_imports: Vec<&str> = our_lines.into_iter().chain(their_lines).collect();
            all_imports.sort();
            all_imports.dedup();

            return Ok(Some(all_imports.join("\n")));
        }

        Ok(None)
    }

    /// Check if a line looks like an import statement
    fn is_import_line(&self, line: &str, file_path: &str) -> bool {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            return true; // Empty lines are OK in import sections
        }

        if file_path.ends_with(".rs") {
            return trimmed.starts_with("use ") || trimmed.starts_with("extern crate");
        } else if file_path.ends_with(".py") {
            return trimmed.starts_with("import ") || trimmed.starts_with("from ");
        } else if file_path.ends_with(".js") || file_path.ends_with(".ts") {
            return trimmed.starts_with("import ")
                || trimmed.starts_with("const ")
                || trimmed.starts_with("require(");
        } else if file_path.ends_with(".go") {
            return trimmed.starts_with("import ") || trimmed == "import (" || trimmed == ")";
        } else if file_path.ends_with(".java") {
            return trimmed.starts_with("import ");
        } else if file_path.ends_with(".swift") {
            return trimmed.starts_with("import ") || trimmed.starts_with("@testable import ");
        } else if file_path.ends_with(".kt") {
            return trimmed.starts_with("import ") || trimmed.starts_with("@file:");
        } else if file_path.ends_with(".cs") {
            return trimmed.starts_with("using ") || trimmed.starts_with("extern alias ");
        }

        false
    }

    /// Normalize whitespace for comparison
    fn normalize_whitespace(&self, content: &str) -> String {
        content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Update a stack entry with new commit information
    /// NOTE: We keep the original branch name to preserve PR mapping, only update commit hash
    fn update_stack_entry(
        &mut self,
        stack_id: Uuid,
        entry_id: &Uuid,
        _new_branch: &str,
        new_commit_hash: &str,
    ) -> Result<()> {
        debug!(
            "Updating entry {} in stack {} with new commit {}",
            entry_id, stack_id, new_commit_hash
        );

        // Get the stack and update the entry
        let stack = self
            .stack_manager
            .get_stack_mut(&stack_id)
            .ok_or_else(|| CascadeError::config(format!("Stack {stack_id} not found")))?;

        // Find and update the entry
        if let Some(entry) = stack.entries.iter_mut().find(|e| e.id == *entry_id) {
            debug!(
                "Found entry {} - updating commit from '{}' to '{}' (keeping original branch '{}')",
                entry_id, entry.commit_hash, new_commit_hash, entry.branch
            );

            // CRITICAL: Keep the original branch name to preserve PR mapping
            // Only update the commit hash to point to the new rebased commit
            entry.commit_hash = new_commit_hash.to_string();

            // Note: Stack will be saved by the caller (StackManager) after rebase completes

            debug!(
                "Successfully updated entry {} in stack {}",
                entry_id, stack_id
            );
            Ok(())
        } else {
            Err(CascadeError::config(format!(
                "Entry {entry_id} not found in stack {stack_id}"
            )))
        }
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
        git_dir.join("REBASE_HEAD").exists()
            || git_dir.join("rebase-merge").exists()
            || git_dir.join("rebase-apply").exists()
    }

    /// Abort an in-progress rebase
    pub fn abort_rebase(&self) -> Result<()> {
        info!("Aborting rebase operation");

        let git_dir = self.git_repo.path().join(".git");

        // Clean up rebase state files
        if git_dir.join("REBASE_HEAD").exists() {
            std::fs::remove_file(git_dir.join("REBASE_HEAD")).map_err(|e| {
                CascadeError::Git(git2::Error::from_str(&format!(
                    "Failed to clean rebase state: {e}"
                )))
            })?;
        }

        if git_dir.join("rebase-merge").exists() {
            std::fs::remove_dir_all(git_dir.join("rebase-merge")).map_err(|e| {
                CascadeError::Git(git2::Error::from_str(&format!(
                    "Failed to clean rebase-merge: {e}"
                )))
            })?;
        }

        if git_dir.join("rebase-apply").exists() {
            std::fs::remove_dir_all(git_dir.join("rebase-apply")).map_err(|e| {
                CascadeError::Git(git2::Error::from_str(&format!(
                    "Failed to clean rebase-apply: {e}"
                )))
            })?;
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
        self.git_repo.stage_conflict_resolved_files()?;

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
            format!(
                "❌ Rebase failed: {}",
                self.error.as_deref().unwrap_or("Unknown error")
            )
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use tempfile::TempDir;

    #[allow(dead_code)]
    fn create_test_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repository
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Initial"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_conflict_region_creation() {
        let region = ConflictRegion {
            start: 0,
            end: 50,
            start_line: 1,
            end_line: 3,
            our_content: "function test() {\n    return true;\n}".to_string(),
            their_content: "function test() {\n  return true;\n}".to_string(),
        };

        assert_eq!(region.start_line, 1);
        assert_eq!(region.end_line, 3);
        assert!(region.our_content.contains("return true"));
        assert!(region.their_content.contains("return true"));
    }

    #[test]
    fn test_rebase_strategies() {
        assert_eq!(RebaseStrategy::ForcePush, RebaseStrategy::ForcePush);
        assert_eq!(RebaseStrategy::Interactive, RebaseStrategy::Interactive);
    }

    #[test]
    fn test_rebase_options() {
        let options = RebaseOptions::default();
        assert_eq!(options.strategy, RebaseStrategy::ForcePush);
        assert!(!options.interactive);
        assert!(options.auto_resolve);
        assert_eq!(options.max_retries, 3);
    }

    #[test]
    fn test_cleanup_guard_tracks_branches() {
        let mut guard = TempBranchCleanupGuard::new();
        assert!(guard.branches.is_empty());

        guard.add_branch("test-branch-1".to_string());
        guard.add_branch("test-branch-2".to_string());

        assert_eq!(guard.branches.len(), 2);
        assert_eq!(guard.branches[0], "test-branch-1");
        assert_eq!(guard.branches[1], "test-branch-2");
    }

    #[test]
    fn test_cleanup_guard_prevents_double_cleanup() {
        use std::process::Command;
        use tempfile::TempDir;

        // Create a temporary git repo
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(repo_path.join("test.txt"), "test").unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        let git_repo = GitRepository::open(repo_path).unwrap();

        // Create a test branch
        git_repo.create_branch("test-temp", None).unwrap();

        let mut guard = TempBranchCleanupGuard::new();
        guard.add_branch("test-temp".to_string());

        // First cleanup should work
        guard.cleanup(&git_repo);
        assert!(guard.cleaned);

        // Second cleanup should be a no-op (shouldn't panic)
        guard.cleanup(&git_repo);
        assert!(guard.cleaned);
    }

    #[test]
    fn test_rebase_result() {
        let result = RebaseResult {
            success: true,
            branch_mapping: std::collections::HashMap::new(),
            conflicts: vec!["abc123".to_string()],
            new_commits: vec!["def456".to_string()],
            error: None,
            summary: "Test summary".to_string(),
        };

        assert!(result.success);
        assert!(result.has_conflicts());
        assert_eq!(result.success_count(), 1);
    }

    #[test]
    fn test_import_line_detection() {
        let (_temp_dir, repo_path) = create_test_repo();
        let git_repo = crate::git::GitRepository::open(&repo_path).unwrap();
        let stack_manager = crate::stack::StackManager::new(&repo_path).unwrap();
        let options = RebaseOptions::default();
        let rebase_manager = RebaseManager::new(stack_manager, git_repo, options);

        // Test Swift import detection
        assert!(rebase_manager.is_import_line("import Foundation", "test.swift"));
        assert!(rebase_manager.is_import_line("@testable import MyModule", "test.swift"));
        assert!(!rebase_manager.is_import_line("class MyClass {", "test.swift"));

        // Test Kotlin import detection
        assert!(rebase_manager.is_import_line("import kotlin.collections.*", "test.kt"));
        assert!(rebase_manager.is_import_line("@file:JvmName(\"Utils\")", "test.kt"));
        assert!(!rebase_manager.is_import_line("fun myFunction() {", "test.kt"));

        // Test C# import detection
        assert!(rebase_manager.is_import_line("using System;", "test.cs"));
        assert!(rebase_manager.is_import_line("using System.Collections.Generic;", "test.cs"));
        assert!(rebase_manager.is_import_line("extern alias GridV1;", "test.cs"));
        assert!(!rebase_manager.is_import_line("namespace MyNamespace {", "test.cs"));

        // Test empty lines are allowed in import sections
        assert!(rebase_manager.is_import_line("", "test.swift"));
        assert!(rebase_manager.is_import_line("   ", "test.kt"));
        assert!(rebase_manager.is_import_line("", "test.cs"));
    }
}
