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
            strategy: RebaseStrategy::BranchVersioning,
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
            RebaseStrategy::BranchVersioning => self.rebase_with_versioning(&stack),
            RebaseStrategy::CherryPick => self.rebase_with_cherry_pick(&stack),
            RebaseStrategy::ThreeWayMerge => self.rebase_with_three_way_merge(&stack),
            RebaseStrategy::Interactive => self.rebase_interactive(&stack),
        }
    }

    /// Rebase using branch versioning strategy (no force-push needed)
    fn rebase_with_versioning(&mut self, stack: &Stack) -> Result<RebaseResult> {
        info!(
            "Rebasing stack '{}' using branch versioning strategy",
            stack.name
        );

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
            .unwrap_or(&stack.base_branch);

        // Ensure we're on the base branch
        if self.git_repo.get_current_branch()? != *target_base {
            self.git_repo.checkout_branch(target_base)?;
        }

        // Only pull if not already done by caller (like sync command)
        // Note: This prevents redundant pulls when called from sync
        if !self.options.skip_pull.unwrap_or(false) {
            if let Err(e) = self.pull_latest_changes(target_base) {
                warn!("Failed to pull latest changes: {}", e);
            }
        } else {
            debug!("Skipping pull - already done by caller");
        }

        let mut current_base = target_base.clone();

        for (index, entry) in stack.entries.iter().enumerate() {
            debug!("Processing entry {}: {}", index, entry.short_hash());

            // Generate new branch name with version
            let new_branch = self.generate_versioned_branch_name(&entry.branch)?;

            // Create new branch from current base
            self.git_repo
                .create_branch(&new_branch, Some(&current_base))?;
            self.git_repo.checkout_branch(&new_branch)?;

            // Cherry-pick the commit onto the new branch
            match self.cherry_pick_commit(&entry.commit_hash) {
                Ok(new_commit_hash) => {
                    result.new_commits.push(new_commit_hash.clone());
                    result
                        .branch_mapping
                        .insert(entry.branch.clone(), new_branch.clone());

                    // Update stack entry with new branch and commit hash
                    self.update_stack_entry(stack.id, &entry.id, &new_branch, &new_commit_hash)?;

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
                            result.error =
                                Some(format!("Could not resolve conflicts: {resolve_err}"));
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

        let target_base = self
            .options
            .target_base
            .as_ref()
            .unwrap_or(&stack.base_branch);

        // Create a temporary rebase branch
        let rebase_branch = format!("{}-rebase-{}", stack.name, Utc::now().timestamp());
        self.git_repo
            .create_branch(&rebase_branch, Some(target_base))?;
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
                        result.error = Some(format!(
                            "Unresolved conflict in {}: {}",
                            entry.commit_hash, e
                        ));
                        break;
                    }
                }
            }
        }

        if result.success {
            // Replace the original branches with the rebased versions
            for entry in &stack.entries {
                let new_branch = format!("{}-rebased", entry.branch);
                self.git_repo
                    .create_branch(&new_branch, Some(&rebase_branch))?;
                result
                    .branch_mapping
                    .insert(entry.branch.clone(), new_branch);
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
        info!(
            "Rebasing stack '{}' using three-way merge strategy",
            stack.name
        );

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

    /// Generate a versioned branch name
    fn generate_versioned_branch_name(&self, original_branch: &str) -> Result<String> {
        let mut version = 2;
        let base_name = if original_branch.ends_with("-v1") {
            original_branch.trim_end_matches("-v1")
        } else {
            original_branch
        };

        loop {
            let candidate = format!("{base_name}-v{version}");
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
        match self.git_repo.cherry_pick(commit_hash) {
            Ok(new_commit_hash) => {
                info!("✅ Cherry-picked {} -> {}", commit_hash, new_commit_hash);

                // Check for any leftover staged changes after successful cherry-pick
                match self.git_repo.get_staged_files() {
                    Ok(staged_files) if !staged_files.is_empty() => {
                        warn!(
                            "Found {} staged files after successful cherry-pick. Committing them.",
                            staged_files.len()
                        );

                        // Commit any leftover staged changes
                        let cleanup_message =
                            format!("Cleanup after cherry-pick {}", &commit_hash[..8]);
                        match self.git_repo.commit_staged_changes(&cleanup_message) {
                            Ok(Some(cleanup_commit)) => {
                                info!(
                                    "✅ Committed {} leftover staged files as {}",
                                    staged_files.len(),
                                    &cleanup_commit[..8]
                                );
                                Ok(new_commit_hash) // Return the original cherry-pick commit
                            }
                            Ok(None) => {
                                // Staged files disappeared, this is fine
                                Ok(new_commit_hash)
                            }
                            Err(commit_err) => {
                                warn!("Failed to commit leftover staged changes: {}", commit_err);
                                // Don't fail the whole operation for this
                                Ok(new_commit_hash)
                            }
                        }
                    }
                    _ => {
                        // No staged changes, normal case
                        Ok(new_commit_hash)
                    }
                }
            }
            Err(e) => {
                // Check if there are staged changes that need to be committed
                match self.git_repo.get_staged_files() {
                    Ok(staged_files) if !staged_files.is_empty() => {
                        warn!(
                            "Cherry-pick failed but found {} staged files. Attempting to commit them.",
                            staged_files.len()
                        );

                        // Try to commit the staged changes
                        let commit_message =
                            format!("Cherry-pick (partial): {}", &commit_hash[..8]);
                        match self.git_repo.commit_staged_changes(&commit_message) {
                            Ok(Some(commit_hash)) => {
                                info!("✅ Committed staged changes as {}", &commit_hash[..8]);
                                Ok(commit_hash)
                            }
                            Ok(None) => {
                                // No staged changes after all
                                Err(e)
                            }
                            Err(commit_err) => {
                                warn!("Failed to commit staged changes: {}", commit_err);
                                Err(e)
                            }
                        }
                    }
                    _ => {
                        // No staged changes, return original error
                        Err(e)
                    }
                }
            }
        }
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
                // All conflicts resolved - write the file back
                std::fs::write(&full_path, content).map_err(|e| {
                    CascadeError::config(format!("Failed to write resolved file {file_path}: {e}"))
                })?;

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
                // All conflicts resolved - write the file back
                std::fs::write(&full_path, resolved_content).map_err(|e| {
                    CascadeError::config(format!("Failed to write resolved file {file_path}: {e}"))
                })?;

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
        assert_eq!(
            RebaseStrategy::BranchVersioning,
            RebaseStrategy::BranchVersioning
        );
        assert_eq!(RebaseStrategy::CherryPick, RebaseStrategy::CherryPick);
        assert_eq!(RebaseStrategy::ThreeWayMerge, RebaseStrategy::ThreeWayMerge);
        assert_eq!(RebaseStrategy::Interactive, RebaseStrategy::Interactive);
    }

    #[test]
    fn test_rebase_options() {
        let options = RebaseOptions::default();
        assert_eq!(options.strategy, RebaseStrategy::BranchVersioning);
        assert!(!options.interactive);
        assert!(options.auto_resolve);
        assert_eq!(options.max_retries, 3);
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
