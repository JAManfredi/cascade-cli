use crate::cli::output::Output;
use crate::errors::Result;
use crate::git::{get_current_repository, ConflictAnalyzer, ConflictType};
use clap::Args;
use std::collections::HashMap;

#[derive(Debug, Args)]
pub struct ConflictsArgs {
    /// Show detailed information about each conflict
    #[arg(long)]
    pub detailed: bool,

    /// Only show conflicts that can be auto-resolved
    #[arg(long)]
    pub auto_only: bool,

    /// Only show conflicts that require manual resolution
    #[arg(long)]
    pub manual_only: bool,

    /// Analyze specific files (if not provided, analyzes all conflicted files)
    #[arg(value_name = "FILE")]
    pub files: Vec<String>,
}

/// Analyze conflicts in the repository
pub async fn run(args: ConflictsArgs) -> Result<()> {
    let git_repo = get_current_repository()?;

    // Check if there are any conflicts
    let has_conflicts = git_repo.has_conflicts()?;

    if !has_conflicts {
        Output::success("No conflicts found in the repository");
        return Ok(());
    }

    // Get conflicted files
    let conflicted_files = if args.files.is_empty() {
        git_repo.get_conflicted_files()?
    } else {
        args.files
    };

    if conflicted_files.is_empty() {
        Output::success("No conflicted files found");
        return Ok(());
    }

    Output::section("Conflict Analysis");

    // Analyze conflicts
    let analyzer = ConflictAnalyzer::new();
    let analysis = analyzer.analyze_conflicts(&conflicted_files, git_repo.path())?;

    // Display summary
    Output::sub_item(format!("Total conflicted files: {}", analysis.files.len()));
    Output::sub_item(format!("Total conflicts: {}", analysis.total_conflicts));
    Output::sub_item(format!(
        "Auto-resolvable: {}",
        analysis.auto_resolvable_count
    ));
    Output::sub_item(format!(
        "Manual resolution needed: {}",
        analysis.total_conflicts - analysis.auto_resolvable_count
    ));

    // Display recommendations
    if !analysis.recommendations.is_empty() {
        Output::section("Recommendations");
        for recommendation in &analysis.recommendations {
            Output::sub_item(recommendation);
        }
    }

    // Display file analysis
    Output::section("File Analysis");

    for file_analysis in &analysis.files {
        // Apply filters
        if args.auto_only && !file_analysis.auto_resolvable {
            continue;
        }
        if args.manual_only && file_analysis.auto_resolvable {
            continue;
        }

        let status_icon = if file_analysis.auto_resolvable {
            "🤖"
        } else {
            "✋"
        };

        let difficulty_desc = match file_analysis.overall_difficulty {
            crate::git::conflict_analysis::ConflictDifficulty::Easy => "Easy",
            crate::git::conflict_analysis::ConflictDifficulty::Medium => "Medium",
            crate::git::conflict_analysis::ConflictDifficulty::Hard => "Hard",
        };

        Output::sub_item(format!(
            "{} {} ({} conflicts, {} difficulty)",
            status_icon,
            file_analysis.file_path,
            file_analysis.conflicts.len(),
            difficulty_desc
        ));

        if args.detailed {
            // Show conflict type breakdown
            let mut type_summary = Vec::new();
            for (conflict_type, count) in &file_analysis.conflict_summary {
                let type_name = match conflict_type {
                    ConflictType::Whitespace => "Whitespace",
                    ConflictType::LineEnding => "Line Endings",
                    ConflictType::PureAddition => "Pure Addition",
                    ConflictType::ImportMerge => "Import Merge",
                    ConflictType::Structural => "Structural",
                    ConflictType::ContentOverlap => "Content Overlap",
                    ConflictType::Complex => "Complex",
                };
                type_summary.push(format!("{type_name}: {count}"));
            }

            if !type_summary.is_empty() {
                Output::sub_item(format!("Types: {}", type_summary.join(", ")));
            }

            // Show individual conflicts
            for (i, conflict) in file_analysis.conflicts.iter().enumerate() {
                let conflict_type = match conflict.conflict_type {
                    ConflictType::Whitespace => "📝 Whitespace",
                    ConflictType::LineEnding => "↩️  Line Endings",
                    ConflictType::PureAddition => "➕ Addition",
                    ConflictType::ImportMerge => "📦 Import",
                    ConflictType::Structural => "🏗️  Structural",
                    ConflictType::ContentOverlap => "🔄 Overlap",
                    ConflictType::Complex => "🔍 Complex",
                };

                let strategy_desc = match &conflict.suggested_strategy {
                    crate::git::conflict_analysis::ResolutionStrategy::TakeOurs => "Take ours",
                    crate::git::conflict_analysis::ResolutionStrategy::TakeTheirs => "Take theirs",
                    crate::git::conflict_analysis::ResolutionStrategy::Merge => "Merge both",
                    crate::git::conflict_analysis::ResolutionStrategy::Custom(desc) => desc,
                    crate::git::conflict_analysis::ResolutionStrategy::Manual => {
                        "Manual resolution"
                    }
                };

                Output::sub_item(format!(
                    "{}. {} (lines {}-{}) - {}",
                    i + 1,
                    conflict_type,
                    conflict.start_line,
                    conflict.end_line,
                    strategy_desc
                ));

                if !conflict.context.is_empty() {
                    Output::sub_item(format!("   Context: {}", conflict.context));
                }
            }
        }
    }

    // Display manual resolution files
    if !analysis.manual_resolution_files.is_empty() {
        Output::section("Files Requiring Manual Resolution");
        for file in &analysis.manual_resolution_files {
            Output::sub_item(format!("✋ {file}"));
        }

        Output::tip("Use 'ca conflicts --detailed' to see specific conflict types");
        Output::tip("Use 'git mergetool' or your editor to resolve manual conflicts");
    }

    // Display auto-resolvable files
    let auto_resolvable_files: Vec<&str> = analysis
        .files
        .iter()
        .filter(|f| f.auto_resolvable)
        .map(|f| f.file_path.as_str())
        .collect();

    if !auto_resolvable_files.is_empty() {
        Output::section("Auto-resolvable Files");
        for file in &auto_resolvable_files {
            Output::sub_item(format!("🤖 {file}"));
        }

        Output::tip("These conflicts can be automatically resolved during rebase/sync");
    }

    Ok(())
}

/// Display conflict statistics
pub fn display_conflict_stats(type_counts: &HashMap<ConflictType, usize>) {
    if type_counts.is_empty() {
        return;
    }

    Output::section("Conflict Types");

    for (conflict_type, count) in type_counts {
        let (icon, description) = match conflict_type {
            ConflictType::Whitespace => ("📝", "Whitespace/formatting differences"),
            ConflictType::LineEnding => ("↩️", "Line ending differences (CRLF vs LF)"),
            ConflictType::PureAddition => ("➕", "Both sides added content"),
            ConflictType::ImportMerge => ("📦", "Import statements that can be merged"),
            ConflictType::Structural => ("🏗️", "Code structure changes"),
            ConflictType::ContentOverlap => ("🔄", "Overlapping content changes"),
            ConflictType::Complex => ("🔍", "Complex conflicts"),
        };

        Output::sub_item(format!("{icon} {description} - {count} conflicts"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    #[test]
    fn test_conflict_stats_display() {
        let mut type_counts = HashMap::new();
        type_counts.insert(ConflictType::Whitespace, 3);
        type_counts.insert(ConflictType::ImportMerge, 2);
        type_counts.insert(ConflictType::Complex, 1);

        // This test just ensures the function doesn't panic
        display_conflict_stats(&type_counts);
    }

    #[test]
    fn test_empty_conflict_stats() {
        let type_counts = HashMap::new();

        // Should handle empty stats gracefully
        display_conflict_stats(&type_counts);
    }
}
