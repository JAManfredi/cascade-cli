use crate::errors::{CascadeError, Result};
use crate::git::{ConflictAnalyzer, GitRepository};
use crate::stack::{Stack, StackManager};
use crate::utils::spinner::{Spinner, SpinnerPrinter};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;
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
    /// Original working branch to restore after rebase (if different from base)
    /// This is critical to prevent updating the base branch when sync checks out to it
    pub original_working_branch: Option<String>,
    /// Optional printer for progress-aware output (e.g., CLI spinners)
    pub progress_printer: Option<SpinnerPrinter>,
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

        tracing::debug!("Cleaning up {} temporary branches", self.branches.len());
        for branch in &self.branches {
            if let Err(e) = git_repo.delete_branch_unsafe(branch) {
                tracing::debug!("Failed to delete temp branch {}: {}", branch, e);
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
            tracing::warn!(
                "{} temporary branches were not cleaned up: {}",
                self.branches.len(),
                self.branches.join(", ")
            );
            tracing::warn!("Run 'ca cleanup' to remove orphaned temporary branches");
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
            original_working_branch: None,
            progress_printer: None,
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

    /// Consume the rebase manager and return the updated stack manager
    pub fn into_stack_manager(self) -> StackManager {
        self.stack_manager
    }

    /// Rebase an entire stack onto a new base
    pub fn rebase_stack(&mut self, stack_id: &Uuid) -> Result<RebaseResult> {
        debug!("Starting rebase for stack {}", stack_id);

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

        // Check if there's an in-progress cherry-pick from a previous failed sync
        if self.has_in_progress_cherry_pick()? {
            return self.handle_in_progress_cherry_pick(stack);
        }

        // Only print section header if no outer spinner exists
        // If progress_printer exists, the caller already has a spinner showing progress
        if self.options.progress_printer.is_none() {
            Output::section(format!("Rebasing stack: {}", stack.name));
            Output::sub_item(format!("Base branch: {}", stack.base_branch));
            Output::sub_item(format!("Entries: {}", stack.entries.len()));
        }

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

        // Use the original working branch passed in options, or detect current branch
        // CRITICAL: sync_stack passes the original branch before it checks out to base
        // This prevents us from thinking we started on the base branch
        let original_branch = self
            .options
            .original_working_branch
            .clone()
            .or_else(|| self.git_repo.get_current_branch().ok());

        // Store original branch for cleanup on early error returns
        let original_branch_for_cleanup = original_branch.clone();

        // SAFETY: Warn if we're starting on the base branch (unusual but valid)
        // This can happen if user manually runs rebase while on base branch
        if let Some(ref orig) = original_branch {
            if orig == &target_base {
                debug!(
                    "Original working branch is base branch '{}' - will skip working branch update",
                    orig
                );
            }
        }

        // Note: Caller (sync_stack) has already checked out base branch when skip_pull=true
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
        let printer = self.options.progress_printer.clone();
        let print_line = |line: &str| {
            if let Some(ref printer) = printer {
                printer.println(line);
            } else {
                println!("{}", line);
            }
        };
        let print_blank = || {
            if let Some(ref printer) = printer {
                printer.println("");
            } else {
                println!();
            }
        };
        let mut temp_branches: Vec<String> = Vec::new(); // Track temp branches for cleanup
        let mut branches_to_push: Vec<(String, String, usize)> = Vec::new(); // (branch_name, pr_number, index)

        // Handle empty stack early
        if entry_count == 0 {
            print_blank();
            Output::info("Stack has no entries yet");
            Output::tip("Use 'ca push' to add commits to this stack");

            result.summary = "Stack is empty".to_string();

            // Print success with summary (consistent with non-empty path)
            print_blank();
            Output::success(&result.summary);

            // Save metadata and return
            self.stack_manager.save_to_disk()?;
            return Ok(result);
        }

        // Check if ALL entries are already correctly based - if so, skip rebase entirely
        let all_up_to_date = stack.entries.iter().all(|entry| {
            self.git_repo
                .is_commit_based_on(&entry.commit_hash, &target_base)
                .unwrap_or(false)
        });

        if all_up_to_date {
            print_blank();
            Output::success("Stack is already up-to-date with base branch");
            result.summary = "Stack is up-to-date".to_string();
            result.success = true;
            return Ok(result);
        }

        // Caller handles title and spinner - just print tree items

        // Phase 1: Rebase all entries locally (libgit2 only - no CLI commands)
        for (index, entry) in stack.entries.iter().enumerate() {
            let original_branch = &entry.branch;

            // Check if this entry is already correctly based on the current base
            // If so, skip rebasing it (avoids creating duplicate commits)
            if self
                .git_repo
                .is_commit_based_on(&entry.commit_hash, &current_base)
                .unwrap_or(false)
            {
                tracing::debug!(
                    "Entry '{}' is already correctly based on '{}', skipping rebase",
                    original_branch,
                    current_base
                );

                // Print tree item immediately and track for push phase
                if let Some(pr_num) = &entry.pull_request_id {
                    let tree_char = if index + 1 == entry_count {
                        "└─"
                    } else {
                        "├─"
                    };
                    print_line(&format!(
                        "   {} {} (PR #{})",
                        tree_char, original_branch, pr_num
                    ));
                    branches_to_push.push((original_branch.clone(), pr_num.clone(), index));
                }

                result
                    .branch_mapping
                    .insert(original_branch.clone(), original_branch.clone());

                // This branch becomes the base for the next entry
                current_base = original_branch.clone();
                continue;
            }

            // Create a temporary branch from the current base
            // This avoids committing directly to protected branches like develop/main
            let temp_branch = format!("{}-temp-{}", original_branch, Utc::now().timestamp());
            temp_branches.push(temp_branch.clone()); // Track for cleanup

            // Create and checkout temp branch - restore original branch on error
            if let Err(e) = self
                .git_repo
                .create_branch(&temp_branch, Some(&current_base))
            {
                // Restore original branch before returning error
                if let Some(ref orig) = original_branch_for_cleanup {
                    let _ = self.git_repo.checkout_branch_unsafe(orig);
                }
                return Err(e);
            }

            if let Err(e) = self.git_repo.checkout_branch_silent(&temp_branch) {
                // Restore original branch before returning error
                if let Some(ref orig) = original_branch_for_cleanup {
                    let _ = self.git_repo.checkout_branch_unsafe(orig);
                }
                return Err(e);
            }

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

                    // Print tree item immediately and track for push phase
                    if let Some(pr_num) = &entry.pull_request_id {
                        let tree_char = if index + 1 == entry_count {
                            "└─"
                        } else {
                            "├─"
                        };
                        print_line(&format!(
                            "   {} {} (PR #{})",
                            tree_char, original_branch, pr_num
                        ));
                        branches_to_push.push((original_branch.clone(), pr_num.clone(), index));
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
                    result.conflicts.push(entry.commit_hash.clone());

                    if !self.options.auto_resolve {
                        print_blank();
                        Output::error(e.to_string());
                        result.success = false;
                        result.error = Some(format!(
                            "Conflict in {}: {}\n\n\
                            MANUAL CONFLICT RESOLUTION REQUIRED\n\
                            =====================================\n\n\
                            Step 1: Analyze conflicts\n\
                            → Run: ca conflicts\n\
                            → This shows which conflicts are in which files\n\n\
                            Step 2: Resolve conflicts in your editor\n\
                            → Open conflicted files and edit them\n\
                            → Remove conflict markers (<<<<<<, ======, >>>>>>)\n\
                            → Keep the code you want\n\
                            → Save the files\n\n\
                            Step 3: Mark conflicts as resolved\n\
                            → Run: git add <resolved-files>\n\
                            → Or: git add -A (to stage all resolved files)\n\n\
                            Step 4: Complete the sync\n\
                            → Run: ca sync\n\
                            → Cascade will detect resolved conflicts and continue\n\n\
                            Alternative: Abort and start over\n\
                            → Run: git cherry-pick --abort\n\
                            → Then: ca sync (starts fresh)\n\n\
                            TIP: Enable auto-resolution for simple conflicts:\n\
                            → Run: ca sync --auto-resolve\n\
                            → Only complex conflicts will require manual resolution",
                            entry.commit_hash, e
                        ));
                        break;
                    }

                    // Try to resolve automatically
                    match self.auto_resolve_conflicts(&entry.commit_hash) {
                        Ok(fully_resolved) => {
                            if !fully_resolved {
                                result.success = false;
                                result.error = Some(format!(
                                    "Conflicts in commit {}\n\n\
                                    To resolve:\n\
                                    1. Fix conflicts in your editor\n\
                                    2. Run: ca sync --continue\n\n\
                                    Or abort:\n\
                                    → Run: git cherry-pick --abort",
                                    &entry.commit_hash[..8]
                                ));
                                break;
                            }

                            // Commit the resolved changes
                            let commit_message =
                                format!("Auto-resolved conflicts in {}", &entry.commit_hash[..8]);

                            // CRITICAL: Check if there are actually changes to commit
                            debug!("Checking staged files before commit");
                            let staged_files = self.git_repo.get_staged_files()?;

                            if staged_files.is_empty() {
                                // NO FILES STAGED! This means auto-resolve didn't actually stage anything
                                // This is the bug - cherry-pick failed, but has_conflicts() returned false
                                // so auto-resolve exited early without staging anything
                                result.success = false;
                                result.error = Some(format!(
                                    "CRITICAL BUG DETECTED: Cherry-pick failed but no files were staged!\n\n\
                                    This indicates a Git state issue after cherry-pick failure.\n\n\
                                    RECOVERY STEPS:\n\
                                    ================\n\n\
                                    Step 1: Check Git status\n\
                                    → Run: git status\n\
                                    → Check if there are any changes in working directory\n\n\
                                    Step 2: Check for conflicts manually\n\
                                    → Run: git diff\n\
                                    → Look for conflict markers (<<<<<<, ======, >>>>>>)\n\n\
                                    Step 3: Abort the cherry-pick\n\
                                    → Run: git cherry-pick --abort\n\n\
                                    Step 4: Report this bug\n\
                                    → This is a known issue we're investigating\n\
                                    → Cherry-pick failed for commit {}\n\
                                    → But Git reported no conflicts and no staged files\n\n\
                                    Step 5: Try manual resolution\n\
                                    → Run: ca sync --no-auto-resolve\n\
                                    → Manually resolve conflicts as they appear",
                                    &entry.commit_hash[..8]
                                ));
                                tracing::error!("CRITICAL - No files staged after auto-resolve!");
                                break;
                            }

                            debug!("{} files staged", staged_files.len());

                            match self.git_repo.commit(&commit_message) {
                                Ok(new_commit_id) => {
                                    debug!(
                                        "Created commit {} with message '{}'",
                                        &new_commit_id[..8],
                                        commit_message
                                    );

                                    Output::success("Auto-resolved conflicts");
                                    result.new_commits.push(new_commit_id.clone());
                                    let rebased_commit_id = new_commit_id;

                                    // Update the original branch to point to this rebased commit
                                    self.git_repo.update_branch_to_commit(
                                        original_branch,
                                        &rebased_commit_id,
                                    )?;

                                    // Print tree item immediately and track for push phase
                                    if let Some(pr_num) = &entry.pull_request_id {
                                        let tree_char = if index + 1 == entry_count {
                                            "└─"
                                        } else {
                                            "├─"
                                        };
                                        println!(
                                            "   {} {} (PR #{})",
                                            tree_char, original_branch, pr_num
                                        );
                                        branches_to_push.push((
                                            original_branch.clone(),
                                            pr_num.clone(),
                                            index,
                                        ));
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
                                Err(commit_err) => {
                                    result.success = false;
                                    result.error = Some(format!(
                                        "Could not commit auto-resolved conflicts: {}\n\n\
                                        This usually means:\n\
                                        - Git index is locked (another process accessing repo)\n\
                                        - File permissions issue\n\
                                        - Disk space issue\n\n\
                                        Recovery:\n\
                                        1. Check if another Git operation is running\n\
                                        2. Run 'rm -f .git/index.lock' if stale lock exists\n\
                                        3. Run 'git status' to check repo state\n\
                                        4. Retry 'ca sync' after fixing the issue",
                                        commit_err
                                    ));
                                    break;
                                }
                            }
                        }
                        Err(resolve_err) => {
                            result.success = false;
                            result.error = Some(format!(
                                "Could not resolve conflicts: {}\n\n\
                                Recovery:\n\
                                1. Check repo state: 'git status'\n\
                                2. If files are staged, commit or reset them: 'git reset --hard HEAD'\n\
                                3. Remove any lock files: 'rm -f .git/index.lock'\n\
                                4. Retry 'ca sync'",
                                resolve_err
                            ));
                            break;
                        }
                    }
                }
            }
        }

        // Cleanup temp branches before returning to original branch
        // Must checkout away from temp branches first
        if !temp_branches.is_empty() {
            // Force checkout to base branch to allow temp branch deletion
            // Use unsafe checkout to bypass safety checks since we know this is cleanup
            if let Err(e) = self.git_repo.checkout_branch_unsafe(&target_base) {
                debug!("Could not checkout base for cleanup: {}", e);
                // If we can't checkout, we can't delete temp branches
                // This is non-critical - temp branches will be cleaned up eventually
            } else {
                // Successfully checked out - now delete temp branches
                for temp_branch in &temp_branches {
                    if let Err(e) = self.git_repo.delete_branch_unsafe(temp_branch) {
                        debug!("Could not delete temp branch {}: {}", temp_branch, e);
                    }
                }
            }
        }

        // Phase 2: Push all branches with PRs to remote (git CLI - after all libgit2 operations)
        // This batch approach prevents index lock conflicts between libgit2 and git CLI
        let pushed_count = branches_to_push.len();
        let skipped_count = entry_count - pushed_count;
        let mut successful_pushes = 0; // Track successful pushes for summary

        if !branches_to_push.is_empty() {
            print_blank();

            // Only create a new spinner if no progress printer exists
            // If we have a printer, the outer spinner is already handling animation
            let push_spinner = if printer.is_none() {
                let branch_word = if pushed_count == 1 {
                    "branch"
                } else {
                    "branches"
                };
                Some(Spinner::new_with_output_below(format!(
                    "Pushing {} updated stack {}",
                    pushed_count, branch_word
                )))
            } else {
                None
            };

            // Push all branches while spinner animates
            let mut push_results = Vec::new();
            for (branch_name, _pr_num, _index) in branches_to_push.iter() {
                let result = self.git_repo.force_push_single_branch_auto(branch_name);
                push_results.push((branch_name.clone(), result));
            }

            // Stop spinner if we created one
            if let Some(spinner) = push_spinner {
                spinner.stop();
            }

            // Now show static results for each push using printer-aware output
            let mut failed_pushes = 0;
            for (index, (branch_name, result)) in push_results.iter().enumerate() {
                match result {
                    Ok(_) => {
                        debug!("Pushed {} successfully", branch_name);
                        successful_pushes += 1;
                        print_line(&format!(
                            "   ✓ Pushed {} ({}/{})",
                            branch_name,
                            index + 1,
                            pushed_count
                        ));
                    }
                    Err(e) => {
                        failed_pushes += 1;
                        print_line(&format!("   ⚠ Could not push '{}': {}", branch_name, e));
                    }
                }
            }

            // If any pushes failed, show recovery instructions
            if failed_pushes > 0 {
                println!(); // Spacing
                Output::warning(format!(
                    "{} branch(es) failed to push to remote",
                    failed_pushes
                ));
                Output::tip("To retry failed pushes, run: ca sync");
            }
        }

        // Update working branch to point to the top of the rebased stack
        // This ensures subsequent `ca push` doesn't re-add old commits
        if let Some(ref orig_branch) = original_branch {
            // CRITICAL: Never update the base branch! Only update working branches
            if orig_branch != &target_base {
                // Get the last entry's branch (top of stack)
                if let Some(last_entry) = stack.entries.last() {
                    let top_branch = &last_entry.branch;

                    // SAFETY CHECK: Detect if working branch has commits beyond the last stack entry
                    // If it does, we need to preserve them - don't force-update the working branch
                    if let (Ok(working_head), Ok(top_commit)) = (
                        self.git_repo.get_branch_head(orig_branch),
                        self.git_repo.get_branch_head(top_branch),
                    ) {
                        // Check if working branch is ahead of top stack entry
                        if working_head != top_commit {
                            // Get commits between top of stack and working branch head
                            if let Ok(commits) = self
                                .git_repo
                                .get_commits_between(&top_commit, &working_head)
                            {
                                if !commits.is_empty() {
                                    // Check if these commits match the stack entry messages
                                    // If so, they're likely old pre-rebase versions, not new work
                                    let stack_messages: Vec<String> = stack
                                        .entries
                                        .iter()
                                        .map(|e| e.message.trim().to_string())
                                        .collect();

                                    let all_match_stack = commits.iter().all(|commit| {
                                        if let Some(msg) = commit.summary() {
                                            stack_messages
                                                .iter()
                                                .any(|stack_msg| stack_msg == msg.trim())
                                        } else {
                                            false
                                        }
                                    });

                                    if all_match_stack && commits.len() == stack.entries.len() {
                                        // These are the old pre-rebase versions of stack entries
                                        // Safe to update working branch to new rebased top
                                        debug!(
                                            "Working branch has old pre-rebase commits (matching stack messages) - safe to update"
                                        );
                                    } else {
                                        // These are truly new commits not in the stack!
                                        Output::error(format!(
                                            "Cannot sync: Working branch '{}' has {} commit(s) not in the stack",
                                            orig_branch,
                                            commits.len()
                                        ));
                                        println!();
                                        Output::sub_item(
                                            "These commits would be lost if we proceed:",
                                        );
                                        for (i, commit) in commits.iter().take(5).enumerate() {
                                            let message =
                                                commit.summary().unwrap_or("(no message)");
                                            Output::sub_item(format!(
                                                "  {}. {} - {}",
                                                i + 1,
                                                &commit.id().to_string()[..8],
                                                message
                                            ));
                                        }
                                        if commits.len() > 5 {
                                            Output::sub_item(format!(
                                                "  ... and {} more",
                                                commits.len() - 5
                                            ));
                                        }
                                        println!();
                                        Output::tip("Add these commits to the stack first:");
                                        Output::bullet("Run: ca stack push");
                                        Output::bullet("Then run: ca sync");
                                        println!();

                                        // Restore original branch before returning error
                                        if let Some(ref orig) = original_branch_for_cleanup {
                                            let _ = self.git_repo.checkout_branch_unsafe(orig);
                                        }

                                        return Err(CascadeError::validation(
                                            format!(
                                                "Working branch '{}' has {} untracked commit(s). Add them to the stack with 'ca stack push' before syncing.",
                                                orig_branch, commits.len()
                                            )
                                        ));
                                    }
                                }
                            }
                        }

                        // Safe to update - working branch matches top of stack or is behind
                        debug!(
                            "Updating working branch '{}' to match top of stack ({})",
                            orig_branch,
                            &top_commit[..8]
                        );

                        if let Err(e) = self
                            .git_repo
                            .update_branch_to_commit(orig_branch, &top_commit)
                        {
                            Output::warning(format!(
                                "Could not update working branch '{}' to top of stack: {}",
                                orig_branch, e
                            ));
                        }
                    }
                }

                // Return to original working branch
                // Use unsafe checkout to force it (we're in cleanup phase, no uncommitted changes)
                if let Err(e) = self.git_repo.checkout_branch_unsafe(orig_branch) {
                    debug!(
                        "Could not return to original branch '{}': {}",
                        orig_branch, e
                    );
                    // Non-critical: User is left on base branch instead of working branch
                }
            } else {
                // User was on base branch - this is unusual but valid
                // Don't update base branch, just checkout back to it
                debug!(
                    "Skipping working branch update - user was on base branch '{}'",
                    orig_branch
                );
                if let Err(e) = self.git_repo.checkout_branch_unsafe(orig_branch) {
                    debug!("Could not return to base branch '{}': {}", orig_branch, e);
                }
            }
        }

        // Build summary message based on actual push success count
        // IMPORTANT: successful_pushes is tracked during the push loop above
        result.summary = if successful_pushes > 0 {
            let pr_plural = if successful_pushes == 1 { "" } else { "s" };
            let entry_plural = if entry_count == 1 { "entry" } else { "entries" };

            if skipped_count > 0 {
                format!(
                    "{} {} rebased ({} PR{} updated, {} not yet submitted)",
                    entry_count, entry_plural, successful_pushes, pr_plural, skipped_count
                )
            } else {
                format!(
                    "{} {} rebased ({} PR{} updated)",
                    entry_count, entry_plural, successful_pushes, pr_plural
                )
            }
        } else if pushed_count > 0 {
            // We attempted pushes but none succeeded
            let entry_plural = if entry_count == 1 { "entry" } else { "entries" };
            format!(
                "{} {} rebased (pushes failed - retry with 'ca sync')",
                entry_count, entry_plural
            )
        } else {
            let plural = if entry_count == 1 { "entry" } else { "entries" };
            format!("{} {} rebased (no PRs to update yet)", entry_count, plural)
        };

        // Display result with proper formatting
        print_blank();
        if result.success {
            Output::success(&result.summary);
        } else {
            // Display error with proper icon
            let error_msg = result
                .error
                .as_deref()
                .unwrap_or("Rebase failed for unknown reason");
            Output::error(error_msg);
        }

        // Save the updated stack metadata to disk
        self.stack_manager.save_to_disk()?;

        // CRITICAL: Return error if rebase failed
        // Don't return Ok(result) with result.success = false - that's confusing!
        if !result.success {
            // Before returning error, try to restore original branch
            if let Some(ref orig_branch) = original_branch {
                if let Err(e) = self.git_repo.checkout_branch_unsafe(orig_branch) {
                    debug!(
                        "Could not return to original branch '{}' after error: {}",
                        orig_branch, e
                    );
                }
            }

            // Include the detailed error message (which contains conflict info)
            let detailed_error = result.error.as_deref().unwrap_or("Rebase failed");
            return Err(CascadeError::Branch(detailed_error.to_string()));
        }

        Ok(result)
    }

    /// Interactive rebase with user input
    fn rebase_interactive(&mut self, stack: &Stack) -> Result<RebaseResult> {
        tracing::debug!("Starting interactive rebase for stack '{}'", stack.name);

        let mut result = RebaseResult {
            success: true,
            branch_mapping: HashMap::new(),
            conflicts: Vec::new(),
            new_commits: Vec::new(),
            error: None,
            summary: String::new(),
        };

        println!("Interactive Rebase for Stack: {}", stack.name);
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
                // CRITICAL: Must commit staged changes - if this fails, user gets checkout warning
                let cleanup_message = format!("Cleanup after cherry-pick {}", &commit_hash[..8]);
                match self.git_repo.commit_staged_changes(&cleanup_message) {
                    Ok(Some(_)) => {
                        debug!(
                            "Committed {} leftover staged files after cherry-pick",
                            staged_files.len()
                        );
                    }
                    Ok(None) => {
                        // Files were unstaged between check and commit - not critical
                        debug!("Staged files were cleared before commit");
                    }
                    Err(e) => {
                        // This is serious - staged files will remain and cause checkout warning
                        tracing::warn!(
                            "Failed to commit {} staged files after cherry-pick: {}. \
                             User may see checkout warning with staged changes.",
                            staged_files.len(),
                            e
                        );
                        // Don't fail the entire rebase for this - but user will need to handle staged files
                    }
                }
            }
        }

        Ok(new_commit_hash)
    }

    /// Attempt to automatically resolve conflicts
    fn auto_resolve_conflicts(&self, commit_hash: &str) -> Result<bool> {
        debug!("Starting auto-resolve for commit {}", commit_hash);

        // Check if there are actually conflicts
        let has_conflicts = self.git_repo.has_conflicts()?;
        debug!("has_conflicts() = {}", has_conflicts);

        // Check if cherry-pick is in progress
        let cherry_pick_head = self.git_repo.path().join(".git").join("CHERRY_PICK_HEAD");
        let cherry_pick_in_progress = cherry_pick_head.exists();

        if !has_conflicts {
            debug!("No conflicts detected by Git index");

            // If cherry-pick is in progress but no conflicts detected, something is wrong
            if cherry_pick_in_progress {
                tracing::debug!(
                    "CHERRY_PICK_HEAD exists but no conflicts in index - aborting cherry-pick"
                );

                // Abort the cherry-pick to clean up
                let _ = std::process::Command::new("git")
                    .args(["cherry-pick", "--abort"])
                    .current_dir(self.git_repo.path())
                    .output();

                return Err(CascadeError::Branch(format!(
                    "Cherry-pick failed for {} but Git index shows no conflicts. \
                     This usually means the cherry-pick was aborted or failed in an unexpected way. \
                     Please try manual resolution.",
                    &commit_hash[..8]
                )));
            }

            return Ok(true);
        }

        let conflicted_files = self.git_repo.get_conflicted_files()?;

        if conflicted_files.is_empty() {
            debug!("Conflicted files list is empty");
            return Ok(true);
        }

        debug!(
            "Found conflicts in {} files: {:?}",
            conflicted_files.len(),
            conflicted_files
        );

        // Use the new conflict analyzer for detailed analysis
        let analysis = self
            .conflict_analyzer
            .analyze_conflicts(&conflicted_files, self.git_repo.path())?;

        debug!(
            "Conflict analysis: {} total conflicts, {} auto-resolvable",
            analysis.total_conflicts, analysis.auto_resolvable_count
        );

        // Display recommendations
        for recommendation in &analysis.recommendations {
            debug!("{}", recommendation);
        }

        let mut resolved_count = 0;
        let mut resolved_files = Vec::new(); // Track which files were actually resolved
        let mut failed_files = Vec::new();

        for file_analysis in &analysis.files {
            debug!(
                "Processing file: {} (auto_resolvable: {}, conflicts: {})",
                file_analysis.file_path,
                file_analysis.auto_resolvable,
                file_analysis.conflicts.len()
            );

            if file_analysis.auto_resolvable {
                match self.resolve_file_conflicts_enhanced(
                    &file_analysis.file_path,
                    &file_analysis.conflicts,
                ) {
                    Ok(ConflictResolution::Resolved) => {
                        resolved_count += 1;
                        resolved_files.push(file_analysis.file_path.clone());
                        debug!("Successfully resolved {}", file_analysis.file_path);
                    }
                    Ok(ConflictResolution::TooComplex) => {
                        debug!(
                            "{} too complex for auto-resolution",
                            file_analysis.file_path
                        );
                        failed_files.push(file_analysis.file_path.clone());
                    }
                    Err(e) => {
                        debug!("Failed to resolve {}: {}", file_analysis.file_path, e);
                        failed_files.push(file_analysis.file_path.clone());
                    }
                }
            } else {
                failed_files.push(file_analysis.file_path.clone());
                debug!(
                    "{} requires manual resolution ({} conflicts)",
                    file_analysis.file_path,
                    file_analysis.conflicts.len()
                );
            }
        }

        if resolved_count > 0 {
            debug!(
                "Resolved {}/{} files",
                resolved_count,
                conflicted_files.len()
            );
            debug!("Resolved files: {:?}", resolved_files);

            // CRITICAL: Only stage files that were successfully resolved
            // This prevents staging files that still have conflict markers
            let file_paths: Vec<&str> = resolved_files.iter().map(|s| s.as_str()).collect();
            debug!("Staging {} files", file_paths.len());
            self.git_repo.stage_files(&file_paths)?;
            debug!("Files staged successfully");
        } else {
            debug!("No files were resolved (resolved_count = 0)");
        }

        // Return true only if ALL conflicts were resolved
        let all_resolved = failed_files.is_empty();

        debug!(
            "all_resolved = {}, failed_files = {:?}",
            all_resolved, failed_files
        );

        if !all_resolved {
            debug!("{} files still need manual resolution", failed_files.len());
        }

        debug!("Returning all_resolved = {}", all_resolved);
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

        tracing::debug!(
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
                debug!(
                    "All conflicts resolved in {}, content length: {} bytes",
                    file_path,
                    content.len()
                );

                // CRITICAL SAFETY CHECK: Don't write empty files!
                if content.trim().is_empty() {
                    tracing::warn!(
                        "SAFETY CHECK: Resolved content for {} is empty! Aborting auto-resolution.",
                        file_path
                    );
                    return Ok(ConflictResolution::TooComplex);
                }

                // SAFETY: Create backup before writing resolved content
                let backup_path = full_path.with_extension("cascade-backup");
                if let Ok(original_content) = std::fs::read_to_string(&full_path) {
                    debug!(
                        "Backup for {} (original: {} bytes, resolved: {} bytes)",
                        file_path,
                        original_content.len(),
                        content.len()
                    );
                    let _ = std::fs::write(&backup_path, original_content);
                }

                // All conflicts resolved - write the file back atomically
                crate::utils::atomic_file::write_string(&full_path, &content)?;

                debug!("Wrote {} bytes to {}", content.len(), file_path);
                return Ok(ConflictResolution::Resolved);
            } else {
                tracing::debug!(
                    "Partially resolved conflicts in {} ({} remaining)",
                    file_path,
                    remaining_conflicts.len()
                );
            }
        }

        Ok(ConflictResolution::TooComplex)
    }

    /// Helper to count whitespace consistency (lower is better)
    #[allow(dead_code)]
    fn count_whitespace_consistency(content: &str) -> usize {
        let mut inconsistencies = 0;
        let lines: Vec<&str> = content.lines().collect();

        for line in &lines {
            // Check for mixed tabs and spaces
            if line.contains('\t') && line.contains(' ') {
                inconsistencies += 1;
            }
        }

        // Penalize for inconsistencies
        lines.len().saturating_sub(inconsistencies)
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
                // SAFETY: Only resolve if the content is truly identical except for whitespace
                // Otherwise, it might be intentional formatting changes
                let our_normalized = conflict
                    .our_content
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                let their_normalized = conflict
                    .their_content
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");

                if our_normalized == their_normalized {
                    // Content is identical - in cherry-pick context, ALWAYS prefer THEIRS
                    // CRITICAL: In cherry-pick, OURS=base branch, THEIRS=commit being applied
                    // We must keep the commit's changes (THEIRS), not the base (OURS)
                    // Otherwise we delete the user's code!
                    Ok(Some(conflict.their_content.clone()))
                } else {
                    // Content differs beyond whitespace - not safe to auto-resolve
                    debug!(
                        "Whitespace conflict has content differences - requires manual resolution"
                    );
                    Ok(None)
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
                // CRITICAL: In cherry-pick, OURS=base, THEIRS=commit being applied
                // We must respect what the commit does (THEIRS), not what the base has (OURS)

                if conflict.our_content.is_empty() && !conflict.their_content.is_empty() {
                    // Base is empty, commit adds content → keep the addition
                    Ok(Some(conflict.their_content.clone()))
                } else if conflict.their_content.is_empty() && !conflict.our_content.is_empty() {
                    // Base has content, commit removes it → keep it removed (empty)
                    Ok(Some(String::new()))
                } else if conflict.our_content.is_empty() && conflict.their_content.is_empty() {
                    // Both empty → keep empty
                    Ok(Some(String::new()))
                } else {
                    // Both sides have content - this could be:
                    // - Duplicate function definitions
                    // - Conflicting logic
                    // - Different implementations of same feature
                    // Too risky to auto-merge - require manual resolution
                    debug!(
                        "PureAddition conflict has content on both sides - requires manual resolution"
                    );
                    Ok(None)
                }
            }
            ConflictType::ImportMerge => {
                // SAFETY: Only merge simple single-line imports
                // Multi-line imports or complex cases require manual resolution

                // Check if all imports are single-line and look like imports
                let our_lines: Vec<&str> = conflict.our_content.lines().collect();
                let their_lines: Vec<&str> = conflict.their_content.lines().collect();

                // Verify all lines look like simple imports (heuristic check)
                let all_simple = our_lines.iter().chain(their_lines.iter()).all(|line| {
                    let trimmed = line.trim();
                    trimmed.starts_with("import ")
                        || trimmed.starts_with("from ")
                        || trimmed.starts_with("use ")
                        || trimmed.starts_with("#include")
                        || trimmed.is_empty()
                });

                if !all_simple {
                    debug!("ImportMerge contains non-import lines - requires manual resolution");
                    return Ok(None);
                }

                // Merge and deduplicate imports
                let mut all_imports: Vec<&str> = our_lines
                    .into_iter()
                    .chain(their_lines)
                    .filter(|line| !line.trim().is_empty())
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

        // Get entry info before mutation
        let entry_exists = stack.entries.iter().any(|e| e.id == *entry_id);

        if entry_exists {
            let old_hash = stack
                .entries
                .iter()
                .find(|e| e.id == *entry_id)
                .map(|e| e.commit_hash.clone())
                .unwrap();

            debug!(
                "Found entry {} - updating commit from '{}' to '{}' (keeping original branch)",
                entry_id, old_hash, new_commit_hash
            );

            // CRITICAL: Keep the original branch name to preserve PR mapping
            // Only update the commit hash to point to the new rebased commit using safe wrapper
            stack
                .update_entry_commit_hash(entry_id, new_commit_hash.to_string())
                .map_err(CascadeError::config)?;

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
        tracing::debug!("Pulling latest changes for branch {}", branch);

        // First try to fetch (this might fail if no remote exists)
        match self.git_repo.fetch() {
            Ok(_) => {
                debug!("Fetch successful");
                // Now try to pull the specific branch
                match self.git_repo.pull(branch) {
                    Ok(_) => {
                        tracing::debug!("Pull completed successfully for {}", branch);
                        Ok(())
                    }
                    Err(e) => {
                        tracing::debug!("Pull failed for {}: {}", branch, e);
                        // Don't fail the entire rebase for pull issues
                        Ok(())
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Fetch failed: {}", e);
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
        tracing::debug!("Aborting rebase operation");

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

        tracing::debug!("Rebase aborted successfully");
        Ok(())
    }

    /// Continue an in-progress rebase after conflict resolution
    pub fn continue_rebase(&self) -> Result<()> {
        tracing::debug!("Continuing rebase operation");

        // Check if there are still conflicts
        if self.git_repo.has_conflicts()? {
            return Err(CascadeError::branch(
                "Cannot continue rebase: there are unresolved conflicts. Resolve conflicts and stage files first.".to_string()
            ));
        }

        // Stage resolved files
        self.git_repo.stage_conflict_resolved_files()?;

        tracing::debug!("Rebase continued successfully");
        Ok(())
    }

    /// Check if there's an in-progress cherry-pick operation
    fn has_in_progress_cherry_pick(&self) -> Result<bool> {
        let git_dir = self.git_repo.path().join(".git");
        Ok(git_dir.join("CHERRY_PICK_HEAD").exists())
    }

    /// Handle resuming an in-progress cherry-pick from a previous failed sync
    fn handle_in_progress_cherry_pick(&mut self, stack: &Stack) -> Result<RebaseResult> {
        use crate::cli::output::Output;

        let git_dir = self.git_repo.path().join(".git");

        Output::section("Resuming in-progress sync");
        println!();
        Output::info("Detected unfinished cherry-pick from previous sync");
        println!();

        // Check if conflicts are resolved
        if self.git_repo.has_conflicts()? {
            let conflicted_files = self.git_repo.get_conflicted_files()?;

            let result = RebaseResult {
                success: false,
                branch_mapping: HashMap::new(),
                conflicts: conflicted_files.clone(),
                new_commits: Vec::new(),
                error: Some(format!(
                    "Cannot continue: {} file(s) still have unresolved conflicts\n\n\
                    MANUAL CONFLICT RESOLUTION REQUIRED\n\
                    =====================================\n\n\
                    Conflicted files:\n{}\n\n\
                    Step 1: Analyze conflicts\n\
                    → Run: ca conflicts\n\
                    → Shows detailed conflict analysis\n\n\
                    Step 2: Resolve conflicts in your editor\n\
                    → Open conflicted files and edit them\n\
                    → Remove conflict markers (<<<<<<, ======, >>>>>>)\n\
                    → Keep the code you want\n\
                    → Save the files\n\n\
                    Step 3: Mark conflicts as resolved\n\
                    → Run: git add <resolved-files>\n\
                    → Or: git add -A (to stage all resolved files)\n\n\
                    Step 4: Complete the sync\n\
                    → Run: ca sync\n\
                    → Cascade will continue from where it left off\n\n\
                    Alternative: Abort and start over\n\
                    → Run: git cherry-pick --abort\n\
                    → Then: ca sync (starts fresh)",
                    conflicted_files.len(),
                    conflicted_files
                        .iter()
                        .map(|f| format!("  - {}", f))
                        .collect::<Vec<_>>()
                        .join("\n")
                )),
                summary: "Sync paused - conflicts need resolution".to_string(),
            };

            return Ok(result);
        }

        // Conflicts are resolved - continue the cherry-pick
        Output::info("Conflicts resolved, continuing cherry-pick...");

        // Stage all resolved files
        self.git_repo.stage_conflict_resolved_files()?;

        // Complete the cherry-pick by committing
        let cherry_pick_msg_file = git_dir.join("CHERRY_PICK_MSG");
        let commit_message = if cherry_pick_msg_file.exists() {
            std::fs::read_to_string(&cherry_pick_msg_file)
                .unwrap_or_else(|_| "Resolved conflicts".to_string())
        } else {
            "Resolved conflicts".to_string()
        };

        match self.git_repo.commit(&commit_message) {
            Ok(_new_commit_id) => {
                Output::success("Cherry-pick completed");

                // Clean up cherry-pick state
                if git_dir.join("CHERRY_PICK_HEAD").exists() {
                    let _ = std::fs::remove_file(git_dir.join("CHERRY_PICK_HEAD"));
                }
                if cherry_pick_msg_file.exists() {
                    let _ = std::fs::remove_file(&cherry_pick_msg_file);
                }

                println!();
                Output::info("Continuing with rest of stack...");
                println!();

                // Now continue with the rest of the rebase
                // We need to restart the full rebase since we don't track which entry we were on
                self.rebase_with_force_push(stack)
            }
            Err(e) => {
                let result = RebaseResult {
                    success: false,
                    branch_mapping: HashMap::new(),
                    conflicts: Vec::new(),
                    new_commits: Vec::new(),
                    error: Some(format!(
                        "Failed to complete cherry-pick: {}\n\n\
                        This usually means:\n\
                        - Git index is locked (another process accessing repo)\n\
                        - File permissions issue\n\
                        - Disk space issue\n\n\
                        Recovery:\n\
                        1. Check if another Git operation is running\n\
                        2. Run 'rm -f .git/index.lock' if stale lock exists\n\
                        3. Run 'git status' to check repo state\n\
                        4. Retry 'ca sync' after fixing the issue\n\n\
                        Or abort and start fresh:\n\
                        → Run: git cherry-pick --abort\n\
                        → Then: ca sync",
                        e
                    )),
                    summary: "Failed to complete cherry-pick".to_string(),
                };

                Ok(result)
            }
        }
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
}
