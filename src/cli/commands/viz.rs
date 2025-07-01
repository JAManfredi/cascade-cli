use crate::errors::{CascadeError, Result};
use crate::git::find_repository_root;
use crate::stack::{Stack, StackManager};
use std::collections::HashMap;
use std::env;
use std::fs;

/// Visualization output formats
#[derive(Debug, Clone)]
pub enum OutputFormat {
    /// ASCII art in terminal
    Ascii,
    /// Mermaid diagram syntax
    Mermaid,
    /// Graphviz DOT notation
    Dot,
    /// PlantUML syntax
    PlantUml,
}

impl OutputFormat {
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "ascii" => Ok(OutputFormat::Ascii),
            "mermaid" => Ok(OutputFormat::Mermaid),
            "dot" | "graphviz" => Ok(OutputFormat::Dot),
            "plantuml" | "puml" => Ok(OutputFormat::PlantUml),
            _ => Err(CascadeError::config(format!("Unknown output format: {s}"))),
        }
    }
}

/// Visualization style options
#[derive(Debug, Clone)]
pub struct VisualizationStyle {
    pub show_commit_hashes: bool,
    pub show_pr_status: bool,
    pub show_branch_names: bool,
    pub compact_mode: bool,
    pub color_coding: bool,
}

impl Default for VisualizationStyle {
    fn default() -> Self {
        Self {
            show_commit_hashes: true,
            show_pr_status: true,
            show_branch_names: true,
            compact_mode: false,
            color_coding: true,
        }
    }
}

/// Stack visualizer
pub struct StackVisualizer {
    style: VisualizationStyle,
}

impl StackVisualizer {
    pub fn new(style: VisualizationStyle) -> Self {
        Self { style }
    }

    /// Generate stack diagram in specified format
    pub fn generate_stack_diagram(&self, stack: &Stack, format: &OutputFormat) -> Result<String> {
        match format {
            OutputFormat::Ascii => self.generate_ascii_diagram(stack),
            OutputFormat::Mermaid => self.generate_mermaid_diagram(stack),
            OutputFormat::Dot => self.generate_dot_diagram(stack),
            OutputFormat::PlantUml => self.generate_plantuml_diagram(stack),
        }
    }

    /// Generate dependency graph showing relationships between entries
    pub fn generate_dependency_graph(
        &self,
        stacks: &[Stack],
        format: &OutputFormat,
    ) -> Result<String> {
        match format {
            OutputFormat::Ascii => self.generate_ascii_dependency_graph(stacks),
            OutputFormat::Mermaid => self.generate_mermaid_dependency_graph(stacks),
            OutputFormat::Dot => self.generate_dot_dependency_graph(stacks),
            OutputFormat::PlantUml => self.generate_plantuml_dependency_graph(stacks),
        }
    }

    fn generate_ascii_diagram(&self, stack: &Stack) -> Result<String> {
        let mut output = String::new();

        // Header
        output.push_str(&format!("📚 Stack: {}\n", stack.name));
        output.push_str(&format!("🌿 Base: {}\n", stack.base_branch));
        if let Some(desc) = &stack.description {
            output.push_str(&format!("📝 Description: {desc}\n"));
        }
        output.push_str(&format!("📊 Status: {:?}\n", stack.status));
        output.push('\n');

        if stack.entries.is_empty() {
            output.push_str("   (empty stack)\n");
            return Ok(output);
        }

        // Stack visualization
        output.push_str("Stack Flow:\n");
        output.push_str("┌─────────────────────────────────────────────────────────────┐\n");

        for (i, entry) in stack.entries.iter().enumerate() {
            let is_last = i == stack.entries.len() - 1;
            let connector = if is_last { "└─" } else { "├─" };
            let vertical = if is_last { "  " } else { "│ " };

            // Status icon
            let status_icon = if entry.pull_request_id.is_some() {
                if entry.is_synced {
                    "✅"
                } else {
                    "📤"
                }
            } else {
                "📝"
            };

            // Main entry line
            output.push_str(&format!("│ {}{} {} ", connector, status_icon, i + 1));

            if self.style.show_commit_hashes {
                output.push_str(&format!("[{}] ", entry.short_hash()));
            }

            output.push_str(&entry.short_message(40));

            if self.style.show_pr_status {
                if let Some(pr_id) = &entry.pull_request_id {
                    output.push_str(&format!(" (PR #{pr_id})"));
                }
            }

            output.push_str(" │\n");

            // Branch info
            if self.style.show_branch_names && !self.style.compact_mode {
                output.push_str(&format!("│ {} 🌿 {:<50} │\n", vertical, entry.branch));
            }

            // Separator for non-compact mode
            if !self.style.compact_mode && !is_last {
                output.push_str(&format!("│ {} {:<50} │\n", vertical, ""));
            }
        }

        output.push_str("└─────────────────────────────────────────────────────────────┘\n");

        // Legend
        output.push_str("\nLegend:\n");
        output.push_str("  📝 Draft  📤 Submitted  ✅ Merged\n");

        Ok(output)
    }

    fn generate_mermaid_diagram(&self, stack: &Stack) -> Result<String> {
        let mut output = String::new();

        output.push_str("graph TD\n");
        output.push_str(&format!("    subgraph \"Stack: {}\"\n", stack.name));
        output.push_str(&format!(
            "        BASE[\"📍 Base: {}\"]\n",
            stack.base_branch
        ));

        if stack.entries.is_empty() {
            output.push_str("        EMPTY[\"(empty stack)\"]\n");
            output.push_str("        BASE --> EMPTY\n");
        } else {
            let mut previous = "BASE".to_string();

            for (i, entry) in stack.entries.iter().enumerate() {
                let node_id = format!("ENTRY{}", i + 1);
                let status_icon = if entry.pull_request_id.is_some() {
                    if entry.is_synced {
                        "✅"
                    } else {
                        "📤"
                    }
                } else {
                    "📝"
                };

                let label = if self.style.compact_mode {
                    format!("{} {}", status_icon, entry.short_message(30))
                } else {
                    format!(
                        "{} {}\\n🌿 {}\\n📋 {}",
                        status_icon,
                        entry.short_message(30),
                        entry.branch,
                        entry.short_hash()
                    )
                };

                output.push_str(&format!("        {node_id}[\"{label}\"]\n"));
                output.push_str(&format!("        {previous} --> {node_id}\n"));

                // Style based on status
                if entry.pull_request_id.is_some() {
                    if entry.is_synced {
                        output.push_str(&format!("        {node_id} --> {node_id}[Merged]\n"));
                        output.push_str(&format!("        class {node_id} merged\n"));
                    } else {
                        output.push_str(&format!("        class {node_id} submitted\n"));
                    }
                } else {
                    output.push_str(&format!("        class {node_id} draft\n"));
                }

                previous = node_id;
            }
        }

        output.push_str("    end\n");

        // Add styling
        output.push('\n');
        output.push_str("    classDef draft fill:#fef3c7,stroke:#d97706,stroke-width:2px\n");
        output.push_str("    classDef submitted fill:#dbeafe,stroke:#2563eb,stroke-width:2px\n");
        output.push_str("    classDef merged fill:#d1fae5,stroke:#059669,stroke-width:2px\n");

        Ok(output)
    }

    fn generate_dot_diagram(&self, stack: &Stack) -> Result<String> {
        let mut output = String::new();

        output.push_str("digraph StackDiagram {\n");
        output.push_str("    rankdir=TB;\n");
        output.push_str("    node [shape=box, style=rounded];\n");
        output.push_str("    edge [arrowhead=open];\n");
        output.push('\n');

        // Subgraph for the stack
        output.push_str("    subgraph cluster_stack {\n");
        output.push_str(&format!("        label=\"Stack: {}\";\n", stack.name));
        output.push_str("        color=blue;\n");

        output.push_str(&format!(
            "        base [label=\"📍 Base: {}\" style=filled fillcolor=lightgray];\n",
            stack.base_branch
        ));

        if stack.entries.is_empty() {
            output.push_str(
                "        empty [label=\"(empty stack)\" style=filled fillcolor=lightgray];\n",
            );
            output.push_str("        base -> empty;\n");
        } else {
            let mut previous = String::from("base");

            for (i, entry) in stack.entries.iter().enumerate() {
                let node_id = format!("entry{}", i + 1);
                let status_icon = if entry.pull_request_id.is_some() {
                    if entry.is_synced {
                        "✅"
                    } else {
                        "📤"
                    }
                } else {
                    "📝"
                };

                let label = format!(
                    "{} {}\\n🌿 {}\\n📋 {}",
                    status_icon,
                    entry.short_message(25).replace("\"", "\\\""),
                    entry.branch,
                    entry.short_hash()
                );

                let color = if entry.pull_request_id.is_some() {
                    if entry.is_synced {
                        "lightgreen"
                    } else {
                        "lightblue"
                    }
                } else {
                    "lightyellow"
                };

                output.push_str(&format!(
                    "        {node_id} [label=\"{label}\" style=filled fillcolor={color}];\n"
                ));
                output.push_str(&format!("        {previous} -> {node_id};\n"));

                previous = node_id;
            }
        }

        output.push_str("    }\n");
        output.push_str("}\n");

        Ok(output)
    }

    fn generate_plantuml_diagram(&self, stack: &Stack) -> Result<String> {
        let mut output = String::new();

        output.push_str("@startuml\n");
        output.push_str("!theme plain\n");
        output.push_str("skinparam backgroundColor #FAFAFA\n");
        output.push_str("skinparam shadowing false\n");
        output.push('\n');

        output.push_str(&format!("title Stack: {}\n", stack.name));
        output.push('\n');

        if stack.entries.is_empty() {
            output.push_str(&format!(
                "rectangle \"📍 Base: {}\" as base #lightgray\n",
                stack.base_branch
            ));
            output.push_str("rectangle \"(empty stack)\" as empty #lightgray\n");
            output.push_str("base --> empty\n");
        } else {
            output.push_str(&format!(
                "rectangle \"📍 Base: {}\" as base #lightgray\n",
                stack.base_branch
            ));

            for (i, entry) in stack.entries.iter().enumerate() {
                let node_id = format!("entry{}", i + 1);
                let status_icon = if entry.pull_request_id.is_some() {
                    if entry.is_synced {
                        "✅"
                    } else {
                        "📤"
                    }
                } else {
                    "📝"
                };

                let color = if entry.pull_request_id.is_some() {
                    if entry.is_synced {
                        "#90EE90"
                    } else {
                        "#ADD8E6"
                    }
                } else {
                    "#FFFFE0"
                };

                let label = format!(
                    "{} {}\\n🌿 {}\\n📋 {}",
                    status_icon,
                    entry.short_message(25),
                    entry.branch,
                    entry.short_hash()
                );

                output.push_str(&format!("rectangle \"{label}\" as {node_id} {color}\n"));

                if i == 0 {
                    output.push_str(&format!("base --> {node_id}\n"));
                } else {
                    output.push_str(&format!("entry{i} --> {node_id}\n"));
                }
            }
        }

        output.push_str("\n@enduml\n");

        Ok(output)
    }

    fn generate_ascii_dependency_graph(&self, stacks: &[Stack]) -> Result<String> {
        let mut output = String::new();

        output.push_str("📊 Stack Dependencies Overview\n");
        output.push_str("═══════════════════════════════\n\n");

        if stacks.is_empty() {
            output.push_str("No stacks found.\n");
            return Ok(output);
        }

        // Group by base branch
        let mut by_base: HashMap<String, Vec<&Stack>> = HashMap::new();
        for stack in stacks {
            by_base
                .entry(stack.base_branch.clone())
                .or_default()
                .push(stack);
        }

        let base_count = by_base.len();
        for (base_branch, base_stacks) in by_base {
            output.push_str(&format!("🌿 Base Branch: {base_branch}\n"));
            output.push_str("┌─────────────────────────────────────────────────────┐\n");

            for (i, stack) in base_stacks.iter().enumerate() {
                let is_last_stack = i == base_stacks.len() - 1;
                let stack_connector = if is_last_stack { "└─" } else { "├─" };
                let stack_vertical = if is_last_stack { "  " } else { "│ " };

                // Stack header
                output.push_str(&format!(
                    "│ {} 📚 {} ({} entries) ",
                    stack_connector,
                    stack.name,
                    stack.entries.len()
                ));

                if stack.is_active {
                    output.push_str("👉 ACTIVE");
                }

                let padding = 50 - (stack.name.len() + stack.entries.len().to_string().len() + 15);
                output.push_str(&" ".repeat(padding.max(0)));
                output.push_str("│\n");

                // Show entries if not in compact mode
                if !self.style.compact_mode && !stack.entries.is_empty() {
                    for (j, entry) in stack.entries.iter().enumerate() {
                        let is_last_entry = j == stack.entries.len() - 1;
                        let entry_connector = if is_last_entry { "└─" } else { "├─" };

                        let status_icon = if entry.pull_request_id.is_some() {
                            if entry.is_synced {
                                "✅"
                            } else {
                                "📤"
                            }
                        } else {
                            "📝"
                        };

                        output.push_str(&format!(
                            "│ {} {} {} {} ",
                            stack_vertical,
                            entry_connector,
                            status_icon,
                            entry.short_message(30)
                        ));

                        let padding = 45 - entry.short_message(30).len();
                        output.push_str(&" ".repeat(padding.max(0)));
                        output.push_str("│\n");
                    }
                }
            }

            output.push_str("└─────────────────────────────────────────────────────┘\n\n");
        }

        // Statistics
        output.push_str("📈 Statistics:\n");
        output.push_str(&format!("  Total stacks: {}\n", stacks.len()));
        output.push_str(&format!("  Base branches: {base_count}\n"));

        let total_entries: usize = stacks.iter().map(|s| s.entries.len()).sum();
        output.push_str(&format!("  Total entries: {total_entries}\n"));

        let active_stacks = stacks.iter().filter(|s| s.is_active).count();
        output.push_str(&format!("  Active stacks: {active_stacks}\n"));

        Ok(output)
    }

    fn generate_mermaid_dependency_graph(&self, stacks: &[Stack]) -> Result<String> {
        let mut output = String::new();

        output.push_str("graph TB\n");
        output.push_str("    subgraph \"Stack Dependencies\"\n");

        // Group by base branch
        let mut by_base: HashMap<String, Vec<&Stack>> = HashMap::new();
        for stack in stacks {
            by_base
                .entry(stack.base_branch.clone())
                .or_default()
                .push(stack);
        }

        for (i, (base_branch, base_stacks)) in by_base.iter().enumerate() {
            let base_id = format!("BASE{i}");
            output.push_str(&format!("        {base_id}[\"🌿 {base_branch}\"]\n"));

            for (j, stack) in base_stacks.iter().enumerate() {
                let stack_id = format!("STACK{i}_{j}");
                let active_marker = if stack.is_active { " 👉" } else { "" };

                output.push_str(&format!(
                    "        {}[\"📚 {} ({} entries){}\"]\n",
                    stack_id,
                    stack.name,
                    stack.entries.len(),
                    active_marker
                ));
                output.push_str(&format!("        {base_id} --> {stack_id}\n"));

                // Add class for active stacks
                if stack.is_active {
                    output.push_str(&format!("        class {stack_id} active\n"));
                }
            }
        }

        output.push_str("    end\n");

        // Add styling
        output.push('\n');
        output.push_str("    classDef active fill:#fef3c7,stroke:#d97706,stroke-width:3px\n");

        Ok(output)
    }

    fn generate_dot_dependency_graph(&self, stacks: &[Stack]) -> Result<String> {
        let mut output = String::new();

        output.push_str("digraph DependencyGraph {\n");
        output.push_str("    rankdir=TB;\n");
        output.push_str("    node [shape=box, style=rounded];\n");
        output.push_str("    edge [arrowhead=open];\n");
        output.push('\n');

        // Group by base branch
        let mut by_base: HashMap<String, Vec<&Stack>> = HashMap::new();
        for stack in stacks {
            by_base
                .entry(stack.base_branch.clone())
                .or_default()
                .push(stack);
        }

        for (i, (base_branch, base_stacks)) in by_base.iter().enumerate() {
            output.push_str(&format!("    subgraph cluster_{i} {{\n"));
            output.push_str(&format!("        label=\"Base: {base_branch}\";\n"));
            output.push_str("        color=blue;\n");

            let base_id = format!("base{i}");
            output.push_str(&format!(
                "        {base_id} [label=\"🌿 {base_branch}\" style=filled fillcolor=lightgray];\n"
            ));

            for (j, stack) in base_stacks.iter().enumerate() {
                let stack_id = format!("stack{i}_{j}");
                let active_marker = if stack.is_active { " 👉" } else { "" };
                let color = if stack.is_active { "gold" } else { "lightblue" };

                output.push_str(&format!(
                    "        {} [label=\"📚 {} ({} entries){}\" style=filled fillcolor={}];\n",
                    stack_id,
                    stack.name,
                    stack.entries.len(),
                    active_marker,
                    color
                ));
                output.push_str(&format!("        {base_id} -> {stack_id};\n"));
            }

            output.push_str("    }\n");
        }

        output.push_str("}\n");

        Ok(output)
    }

    fn generate_plantuml_dependency_graph(&self, stacks: &[Stack]) -> Result<String> {
        let mut output = String::new();

        output.push_str("@startuml\n");
        output.push_str("!theme plain\n");
        output.push_str("skinparam backgroundColor #FAFAFA\n");
        output.push('\n');

        output.push_str("title Stack Dependencies\n");
        output.push('\n');

        // Group by base branch
        let mut by_base: HashMap<String, Vec<&Stack>> = HashMap::new();
        for stack in stacks {
            by_base
                .entry(stack.base_branch.clone())
                .or_default()
                .push(stack);
        }

        for (i, (base_branch, base_stacks)) in by_base.iter().enumerate() {
            let base_id = format!("base{i}");
            output.push_str(&format!(
                "rectangle \"🌿 {base_branch}\" as {base_id} #lightgray\n"
            ));

            for (j, stack) in base_stacks.iter().enumerate() {
                let stack_id = format!("stack{i}_{j}");
                let active_marker = if stack.is_active { " 👉" } else { "" };
                let color = if stack.is_active {
                    "#FFD700"
                } else {
                    "#ADD8E6"
                };

                output.push_str(&format!(
                    "rectangle \"📚 {} ({} entries){}\" as {} {}\n",
                    stack.name,
                    stack.entries.len(),
                    active_marker,
                    stack_id,
                    color
                ));
                output.push_str(&format!("{base_id} --> {stack_id}\n"));
            }
        }

        output.push_str("\n@enduml\n");

        Ok(output)
    }
}

/// Visualize a specific stack
pub async fn show_stack(
    stack_name: Option<String>,
    format: Option<String>,
    output_file: Option<String>,
    compact: bool,
    no_colors: bool,
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let manager = StackManager::new(&repo_root)?;

    let stack = if let Some(name) = stack_name {
        manager
            .get_stack_by_name(&name)
            .ok_or_else(|| CascadeError::config(format!("Stack '{name}' not found")))?
    } else {
        manager.get_active_stack().ok_or_else(|| {
            CascadeError::config("No active stack. Use 'ca stack list' to see available stacks")
        })?
    };

    let output_format = format
        .as_ref()
        .map(|f| OutputFormat::from_str(f))
        .transpose()?
        .unwrap_or(OutputFormat::Ascii);

    let style = VisualizationStyle {
        compact_mode: compact,
        color_coding: !no_colors,
        ..Default::default()
    };

    let visualizer = StackVisualizer::new(style);
    let diagram = visualizer.generate_stack_diagram(stack, &output_format)?;

    if let Some(file_path) = output_file {
        fs::write(&file_path, diagram).map_err(|e| {
            CascadeError::config(format!("Failed to write to file '{file_path}': {e}"))
        })?;
        println!("✅ Stack diagram saved to: {file_path}");
    } else {
        println!("{diagram}");
    }

    Ok(())
}

/// Visualize all stacks and their dependencies
pub async fn show_dependencies(
    format: Option<String>,
    output_file: Option<String>,
    compact: bool,
    no_colors: bool,
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let manager = StackManager::new(&repo_root)?;
    let stacks = manager.get_all_stacks_objects()?;

    if stacks.is_empty() {
        println!("No stacks found. Create one with: ca stack create <name>");
        return Ok(());
    }

    let output_format = format
        .as_ref()
        .map(|f| OutputFormat::from_str(f))
        .transpose()?
        .unwrap_or(OutputFormat::Ascii);

    let style = VisualizationStyle {
        compact_mode: compact,
        color_coding: !no_colors,
        ..Default::default()
    };

    let visualizer = StackVisualizer::new(style);
    let diagram = visualizer.generate_dependency_graph(&stacks, &output_format)?;

    if let Some(file_path) = output_file {
        fs::write(&file_path, diagram).map_err(|e| {
            CascadeError::config(format!("Failed to write to file '{file_path}': {e}"))
        })?;
        println!("✅ Dependency graph saved to: {file_path}");
    } else {
        println!("{diagram}");
    }

    Ok(())
}
