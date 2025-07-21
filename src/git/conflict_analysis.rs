use crate::errors::{CascadeError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of conflict detected
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ConflictType {
    /// Only whitespace or formatting differences
    Whitespace,
    /// Only line ending differences (CRLF vs LF)
    LineEnding,
    /// Both sides added lines without overlapping changes
    PureAddition,
    /// Import/dependency statements that can be merged
    ImportMerge,
    /// Code structure changes (functions, classes, etc.)
    Structural,
    /// Content changes that overlap
    ContentOverlap,
    /// Complex conflicts requiring manual resolution
    Complex,
}

/// Difficulty level for resolving a conflict
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConflictDifficulty {
    /// Can be automatically resolved
    Easy,
    /// Requires simple user input
    Medium,
    /// Requires careful manual resolution
    Hard,
}

/// Strategy for resolving a conflict
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResolutionStrategy {
    /// Use "our" version
    TakeOurs,
    /// Use "their" version
    TakeTheirs,
    /// Merge both versions
    Merge,
    /// Apply custom resolution logic
    Custom(String),
    /// Cannot be automatically resolved
    Manual,
}

/// Represents a conflict region in a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictRegion {
    /// File path where conflict occurs
    pub file_path: String,
    /// Byte position where conflict starts
    pub start_pos: usize,
    /// Byte position where conflict ends
    pub end_pos: usize,
    /// Line number where conflict starts
    pub start_line: usize,
    /// Line number where conflict ends
    pub end_line: usize,
    /// Content from "our" side (before separator)
    pub our_content: String,
    /// Content from "their" side (after separator)
    pub their_content: String,
    /// Type of conflict detected
    pub conflict_type: ConflictType,
    /// Difficulty level for resolution
    pub difficulty: ConflictDifficulty,
    /// Suggested resolution strategy
    pub suggested_strategy: ResolutionStrategy,
    /// Additional context or explanation
    pub context: String,
}

/// Analysis result for a file with conflicts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConflictAnalysis {
    /// Path to the file
    pub file_path: String,
    /// List of conflict regions in the file
    pub conflicts: Vec<ConflictRegion>,
    /// Overall difficulty assessment
    pub overall_difficulty: ConflictDifficulty,
    /// Whether all conflicts can be auto-resolved
    pub auto_resolvable: bool,
    /// Summary of conflict types
    pub conflict_summary: HashMap<ConflictType, usize>,
}

/// Complete analysis of all conflicts in a rebase/merge operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictAnalysis {
    /// Analysis for each conflicted file
    pub files: Vec<FileConflictAnalysis>,
    /// Total number of conflicts
    pub total_conflicts: usize,
    /// Number of auto-resolvable conflicts
    pub auto_resolvable_count: usize,
    /// Files requiring manual resolution
    pub manual_resolution_files: Vec<String>,
    /// Recommended next steps
    pub recommendations: Vec<String>,
}

/// Analyzes conflicts in files and provides resolution guidance
pub struct ConflictAnalyzer {
    /// Known patterns for different file types
    file_patterns: HashMap<String, Vec<String>>,
}

impl ConflictAnalyzer {
    /// Create a new conflict analyzer
    pub fn new() -> Self {
        let mut file_patterns = HashMap::new();

        // Rust patterns
        file_patterns.insert(
            "rs".to_string(),
            vec![
                "use ".to_string(),
                "extern crate ".to_string(),
                "mod ".to_string(),
                "fn ".to_string(),
                "struct ".to_string(),
                "enum ".to_string(),
                "impl ".to_string(),
            ],
        );

        // Python patterns
        file_patterns.insert(
            "py".to_string(),
            vec![
                "import ".to_string(),
                "from ".to_string(),
                "def ".to_string(),
                "class ".to_string(),
                "__init__".to_string(),
            ],
        );

        // JavaScript/TypeScript patterns
        file_patterns.insert(
            "js".to_string(),
            vec![
                "import ".to_string(),
                "export ".to_string(),
                "const ".to_string(),
                "function ".to_string(),
                "class ".to_string(),
            ],
        );

        file_patterns.insert(
            "ts".to_string(),
            vec![
                "import ".to_string(),
                "export ".to_string(),
                "interface ".to_string(),
                "type ".to_string(),
                "function ".to_string(),
                "class ".to_string(),
            ],
        );

        Self { file_patterns }
    }

    /// Analyze all conflicts in a file
    pub fn analyze_file(&self, file_path: &str, content: &str) -> Result<FileConflictAnalysis> {
        let conflicts = self.parse_conflict_markers(file_path, content)?;

        let mut conflict_summary = HashMap::new();
        let mut auto_resolvable_count = 0;

        for conflict in &conflicts {
            *conflict_summary
                .entry(conflict.conflict_type.clone())
                .or_insert(0) += 1;

            if conflict.difficulty == ConflictDifficulty::Easy {
                auto_resolvable_count += 1;
            }
        }

        let overall_difficulty = self.assess_overall_difficulty(&conflicts);
        let auto_resolvable = auto_resolvable_count == conflicts.len();

        Ok(FileConflictAnalysis {
            file_path: file_path.to_string(),
            conflicts,
            overall_difficulty,
            auto_resolvable,
            conflict_summary,
        })
    }

    /// Analyze conflicts across multiple files
    pub fn analyze_conflicts(
        &self,
        conflicted_files: &[String],
        repo_path: &std::path::Path,
    ) -> Result<ConflictAnalysis> {
        let mut file_analyses = Vec::new();
        let mut total_conflicts = 0;
        let mut auto_resolvable_count = 0;
        let mut manual_resolution_files = Vec::new();

        for file_path in conflicted_files {
            let full_path = repo_path.join(file_path);
            let content = std::fs::read_to_string(&full_path)
                .map_err(|e| CascadeError::config(format!("Failed to read {file_path}: {e}")))?;

            let analysis = self.analyze_file(file_path, &content)?;

            total_conflicts += analysis.conflicts.len();
            auto_resolvable_count += analysis
                .conflicts
                .iter()
                .filter(|c| c.difficulty == ConflictDifficulty::Easy)
                .count();

            if !analysis.auto_resolvable {
                manual_resolution_files.push(file_path.clone());
            }

            file_analyses.push(analysis);
        }

        let recommendations = self.generate_recommendations(&file_analyses);

        Ok(ConflictAnalysis {
            files: file_analyses,
            total_conflicts,
            auto_resolvable_count,
            manual_resolution_files,
            recommendations,
        })
    }

    /// Parse conflict markers from file content
    fn parse_conflict_markers(
        &self,
        file_path: &str,
        content: &str,
    ) -> Result<Vec<ConflictRegion>> {
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
                        separator_line = Some(j);
                    } else if line.starts_with(">>>>>>>") {
                        end_line = Some(j);
                        break;
                    }
                }

                if let (Some(sep), Some(end)) = (separator_line, end_line) {
                    // Calculate byte positions
                    let start_pos = lines[..i].iter().map(|l| l.len() + 1).sum::<usize>();
                    let end_pos = lines[..=end].iter().map(|l| l.len() + 1).sum::<usize>();

                    let our_content = lines[(i + 1)..sep].join("\n");
                    let their_content = lines[(sep + 1)..end].join("\n");

                    // Analyze this conflict
                    let conflict_region = self.analyze_conflict_region(
                        file_path,
                        start_pos,
                        end_pos,
                        start_line,
                        end + 1,
                        &our_content,
                        &their_content,
                    )?;

                    conflicts.push(conflict_region);
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

    /// Analyze a single conflict region
    #[allow(clippy::too_many_arguments)]
    fn analyze_conflict_region(
        &self,
        file_path: &str,
        start_pos: usize,
        end_pos: usize,
        start_line: usize,
        end_line: usize,
        our_content: &str,
        their_content: &str,
    ) -> Result<ConflictRegion> {
        let conflict_type = self.classify_conflict_type(file_path, our_content, their_content);
        let difficulty = self.assess_difficulty(&conflict_type, our_content, their_content);
        let suggested_strategy =
            self.suggest_resolution_strategy(&conflict_type, our_content, their_content);
        let context = self.generate_context(&conflict_type, our_content, their_content);

        Ok(ConflictRegion {
            file_path: file_path.to_string(),
            start_pos,
            end_pos,
            start_line,
            end_line,
            our_content: our_content.to_string(),
            their_content: their_content.to_string(),
            conflict_type,
            difficulty,
            suggested_strategy,
            context,
        })
    }

    /// Classify the type of conflict
    fn classify_conflict_type(
        &self,
        file_path: &str,
        our_content: &str,
        their_content: &str,
    ) -> ConflictType {
        // Check for whitespace-only differences
        if self.normalize_whitespace(our_content) == self.normalize_whitespace(their_content) {
            return ConflictType::Whitespace;
        }

        // Check for line ending differences
        if self.normalize_line_endings(our_content) == self.normalize_line_endings(their_content) {
            return ConflictType::LineEnding;
        }

        // Check for pure additions
        if our_content.is_empty() || their_content.is_empty() {
            return ConflictType::PureAddition;
        }

        // Check for import conflicts
        if self.is_import_conflict(file_path, our_content, their_content) {
            return ConflictType::ImportMerge;
        }

        // Check for structural changes
        if self.is_structural_conflict(file_path, our_content, their_content) {
            return ConflictType::Structural;
        }

        // Check for content overlap
        if self.has_content_overlap(our_content, their_content) {
            return ConflictType::ContentOverlap;
        }

        // Default to complex
        ConflictType::Complex
    }

    /// Assess difficulty level for resolution
    fn assess_difficulty(
        &self,
        conflict_type: &ConflictType,
        our_content: &str,
        their_content: &str,
    ) -> ConflictDifficulty {
        match conflict_type {
            ConflictType::Whitespace | ConflictType::LineEnding => ConflictDifficulty::Easy,
            ConflictType::PureAddition => {
                if our_content.lines().count() <= 5 && their_content.lines().count() <= 5 {
                    ConflictDifficulty::Easy
                } else {
                    ConflictDifficulty::Medium
                }
            }
            ConflictType::ImportMerge => ConflictDifficulty::Easy,
            ConflictType::Structural => ConflictDifficulty::Medium,
            ConflictType::ContentOverlap => ConflictDifficulty::Medium,
            ConflictType::Complex => ConflictDifficulty::Hard,
        }
    }

    /// Suggest resolution strategy
    fn suggest_resolution_strategy(
        &self,
        conflict_type: &ConflictType,
        our_content: &str,
        their_content: &str,
    ) -> ResolutionStrategy {
        match conflict_type {
            ConflictType::Whitespace => {
                if our_content.trim().len() >= their_content.trim().len() {
                    ResolutionStrategy::TakeOurs
                } else {
                    ResolutionStrategy::TakeTheirs
                }
            }
            ConflictType::LineEnding => {
                ResolutionStrategy::Custom("Normalize to Unix line endings".to_string())
            }
            ConflictType::PureAddition => ResolutionStrategy::Merge,
            ConflictType::ImportMerge => {
                ResolutionStrategy::Custom("Sort and merge imports".to_string())
            }
            ConflictType::Structural => ResolutionStrategy::Manual,
            ConflictType::ContentOverlap => ResolutionStrategy::Manual,
            ConflictType::Complex => ResolutionStrategy::Manual,
        }
    }

    /// Generate context description
    fn generate_context(
        &self,
        conflict_type: &ConflictType,
        our_content: &str,
        their_content: &str,
    ) -> String {
        match conflict_type {
            ConflictType::Whitespace => "Conflicts only in whitespace/formatting".to_string(),
            ConflictType::LineEnding => "Conflicts only in line endings (CRLF vs LF)".to_string(),
            ConflictType::PureAddition => {
                format!(
                    "Both sides added content: {} vs {} lines",
                    our_content.lines().count(),
                    their_content.lines().count()
                )
            }
            ConflictType::ImportMerge => "Import statements that can be merged".to_string(),
            ConflictType::Structural => {
                "Changes to code structure (functions, classes, etc.)".to_string()
            }
            ConflictType::ContentOverlap => "Overlapping changes to the same content".to_string(),
            ConflictType::Complex => "Complex conflicts requiring manual review".to_string(),
        }
    }

    /// Check if this is an import conflict
    fn is_import_conflict(&self, file_path: &str, our_content: &str, their_content: &str) -> bool {
        let extension = file_path.split('.').next_back().unwrap_or("");

        if let Some(patterns) = self.file_patterns.get(extension) {
            let our_lines: Vec<&str> = our_content.lines().collect();
            let their_lines: Vec<&str> = their_content.lines().collect();

            let our_imports = our_lines.iter().all(|line| {
                let trimmed = line.trim();
                trimmed.is_empty() || patterns.iter().any(|pattern| trimmed.starts_with(pattern))
            });

            let their_imports = their_lines.iter().all(|line| {
                let trimmed = line.trim();
                trimmed.is_empty() || patterns.iter().any(|pattern| trimmed.starts_with(pattern))
            });

            return our_imports && their_imports;
        }

        false
    }

    /// Check if this is a structural conflict
    fn is_structural_conflict(
        &self,
        file_path: &str,
        our_content: &str,
        their_content: &str,
    ) -> bool {
        let extension = file_path.split('.').next_back().unwrap_or("");

        if let Some(patterns) = self.file_patterns.get(extension) {
            let structural_keywords = patterns
                .iter()
                .filter(|p| {
                    !p.starts_with("import") && !p.starts_with("use") && !p.starts_with("from")
                })
                .collect::<Vec<_>>();

            let our_has_structure = our_content.lines().any(|line| {
                structural_keywords
                    .iter()
                    .any(|keyword| line.trim().starts_with(*keyword))
            });

            let their_has_structure = their_content.lines().any(|line| {
                structural_keywords
                    .iter()
                    .any(|keyword| line.trim().starts_with(*keyword))
            });

            return our_has_structure || their_has_structure;
        }

        false
    }

    /// Check if there's content overlap
    fn has_content_overlap(&self, our_content: &str, their_content: &str) -> bool {
        let our_lines: Vec<&str> = our_content.lines().collect();
        let their_lines: Vec<&str> = their_content.lines().collect();

        // Check for common lines
        for our_line in &our_lines {
            if their_lines.contains(our_line) && !our_line.trim().is_empty() {
                return true;
            }
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

    /// Normalize line endings
    fn normalize_line_endings(&self, content: &str) -> String {
        content.replace("\r\n", "\n").replace('\r', "\n")
    }

    /// Assess overall difficulty for a file
    fn assess_overall_difficulty(&self, conflicts: &[ConflictRegion]) -> ConflictDifficulty {
        if conflicts.is_empty() {
            return ConflictDifficulty::Easy;
        }

        let has_hard = conflicts
            .iter()
            .any(|c| c.difficulty == ConflictDifficulty::Hard);
        let has_medium = conflicts
            .iter()
            .any(|c| c.difficulty == ConflictDifficulty::Medium);

        if has_hard {
            ConflictDifficulty::Hard
        } else if has_medium {
            ConflictDifficulty::Medium
        } else {
            ConflictDifficulty::Easy
        }
    }

    /// Generate recommendations for resolving conflicts
    fn generate_recommendations(&self, file_analyses: &[FileConflictAnalysis]) -> Vec<String> {
        let mut recommendations = Vec::new();

        let auto_resolvable_files = file_analyses.iter().filter(|f| f.auto_resolvable).count();

        if auto_resolvable_files > 0 {
            recommendations.push(format!(
                "🤖 {auto_resolvable_files} file(s) can be automatically resolved"
            ));
        }

        let manual_files = file_analyses.iter().filter(|f| !f.auto_resolvable).count();

        if manual_files > 0 {
            recommendations.push(format!(
                "✋ {manual_files} file(s) require manual resolution"
            ));
        }

        // Count conflict types
        let mut type_counts = HashMap::new();
        for analysis in file_analyses {
            for (conflict_type, count) in &analysis.conflict_summary {
                *type_counts.entry(conflict_type.clone()).or_insert(0) += count;
            }
        }

        for (conflict_type, count) in type_counts {
            match conflict_type {
                ConflictType::Whitespace => {
                    recommendations.push(format!(
                        "🔧 {count} whitespace conflicts can be auto-formatted"
                    ));
                }
                ConflictType::ImportMerge => {
                    recommendations.push(format!(
                        "📦 {count} import conflicts can be merged automatically"
                    ));
                }
                ConflictType::Structural => {
                    recommendations.push(format!(
                        "🏗️  {count} structural conflicts need careful review"
                    ));
                }
                ConflictType::Complex => {
                    recommendations.push(format!(
                        "🔍 {count} complex conflicts require manual resolution"
                    ));
                }
                _ => {}
            }
        }

        recommendations
    }
}

impl Default for ConflictAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_type_classification() {
        let analyzer = ConflictAnalyzer::new();

        // Test whitespace conflict
        let our_content = "function test() {\n    return true;\n}";
        let their_content = "function test() {\n  return true;\n}";
        let conflict_type = analyzer.classify_conflict_type("test.js", our_content, their_content);
        assert_eq!(conflict_type, ConflictType::Whitespace);

        // Test pure addition
        let our_content = "";
        let their_content = "import React from 'react';";
        let conflict_type = analyzer.classify_conflict_type("test.js", our_content, their_content);
        assert_eq!(conflict_type, ConflictType::PureAddition);

        // Test import merge
        let our_content = "import { useState } from 'react';";
        let their_content = "import { useEffect } from 'react';";
        let conflict_type = analyzer.classify_conflict_type("test.js", our_content, their_content);
        assert_eq!(conflict_type, ConflictType::ImportMerge);
    }

    #[test]
    fn test_difficulty_assessment() {
        let analyzer = ConflictAnalyzer::new();

        assert_eq!(
            analyzer.assess_difficulty(&ConflictType::Whitespace, "", ""),
            ConflictDifficulty::Easy
        );

        assert_eq!(
            analyzer.assess_difficulty(&ConflictType::Complex, "", ""),
            ConflictDifficulty::Hard
        );

        assert_eq!(
            analyzer.assess_difficulty(&ConflictType::Structural, "", ""),
            ConflictDifficulty::Medium
        );
    }

    #[test]
    fn test_conflict_marker_parsing() {
        let analyzer = ConflictAnalyzer::new();
        let content = r#"
line before conflict
<<<<<<< HEAD
our content
=======
their content
>>>>>>> branch
line after conflict
"#;

        let conflicts = analyzer
            .parse_conflict_markers("test.txt", content)
            .unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].our_content, "our content");
        assert_eq!(conflicts[0].their_content, "their content");
    }

    #[test]
    fn test_import_conflict_detection() {
        let analyzer = ConflictAnalyzer::new();

        // Rust imports
        assert!(analyzer.is_import_conflict(
            "main.rs",
            "use std::collections::HashMap;",
            "use std::collections::HashSet;"
        ));

        // Python imports
        assert!(analyzer.is_import_conflict("main.py", "import os", "import sys"));

        // Not an import conflict
        assert!(!analyzer.is_import_conflict("main.rs", "fn main() {}", "fn test() {}"));
    }
}
