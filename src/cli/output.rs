use console::{style, Emoji};
use std::fmt::Display;

/// Centralized output formatting utilities for consistent CLI presentation
pub struct Output;

impl Output {
    /// Print a success message with checkmark
    pub fn success<T: Display>(message: T) {
        println!("{} {}", style("✓").green(), message);
    }

    /// Print an error message with X mark
    pub fn error<T: Display>(message: T) {
        println!("{} {}", style("✗").red(), message);
    }

    /// Print a warning message with warning emoji
    pub fn warning<T: Display>(message: T) {
        println!("{} {}", style("⚠").yellow(), message);
    }

    /// Print an info message with info emoji
    pub fn info<T: Display>(message: T) {
        println!("{} {}", style("ℹ").cyan(), message);
    }

    /// Print a sub-item with arrow prefix
    pub fn sub_item<T: Display>(message: T) {
        println!("  {} {}", style("→").dim(), message);
    }

    /// Print a bullet point
    pub fn bullet<T: Display>(message: T) {
        println!("  {} {}", style("•").dim(), message);
    }

    /// Print a section header
    pub fn section<T: Display>(title: T) {
        println!("\n{}", style(title).bold().underlined());
    }

    /// Print a tip/suggestion
    pub fn tip<T: Display>(message: T) {
        println!("{} {}", style("TIP:").cyan(), style(message).dim());
    }

    /// Print progress indicator
    pub fn progress<T: Display>(message: T) {
        println!("{} {}", style("→").cyan(), message);
    }

    /// Print a divider line
    pub fn divider() {
        println!("{}", style("─".repeat(50)).dim());
    }

    /// Print stack information in a formatted way
    pub fn stack_info(
        name: &str,
        id: &str,
        base_branch: &str,
        working_branch: Option<&str>,
        is_active: bool,
    ) {
        Self::success(format!("Created stack '{name}'"));
        Self::sub_item(format!("Stack ID: {}", style(id).dim()));
        Self::sub_item(format!("Base branch: {}", style(base_branch).cyan()));

        if let Some(working) = working_branch {
            Self::sub_item(format!("Working branch: {}", style(working).cyan()));
        }

        if is_active {
            Self::sub_item(format!("Status: {}", style("Active").green()));
        }
    }

    /// Print next steps guidance
    pub fn next_steps(steps: &[&str]) {
        println!();
        Self::tip("Next steps:");
        for step in steps {
            Self::bullet(step);
        }
    }

    /// Print a command example
    pub fn command_example<T: Display>(command: T) {
        println!("  {}", style(command).yellow());
    }

    /// Print a check start message
    pub fn check_start<T: Display>(message: T) {
        println!("\n{} {}", style("🔍").bright(), style(message).bold());
    }

    /// Print a solution message
    pub fn solution<T: Display>(message: T) {
        println!("     {}: {}", style("Solution").yellow(), message);
    }

    /// Print a numbered item
    pub fn numbered_item<T: Display>(number: usize, message: T) {
        println!("  {}. {}", style(number).cyan(), message);
    }

    /// Print empty line for spacing
    pub fn spacing() {
        println!();
    }
}

/// Emojis for different contexts
pub struct Emojis;

impl Emojis {
    pub const SUCCESS: Emoji<'_, '_> = Emoji("✓", "OK");
    pub const ERROR: Emoji<'_, '_> = Emoji("✗", "ERROR");
    pub const WARNING: Emoji<'_, '_> = Emoji("⚠", "WARNING");
    pub const INFO: Emoji<'_, '_> = Emoji("ℹ", "INFO");
    pub const TIP: Emoji<'_, '_> = Emoji("💡", "TIP");
    pub const ROCKET: Emoji<'_, '_> = Emoji("🚀", "ROCKET");
    pub const SEARCH: Emoji<'_, '_> = Emoji("🔍", "SEARCH");
    pub const UPLOAD: Emoji<'_, '_> = Emoji("📤", "UPLOAD");
    pub const STACK: Emoji<'_, '_> = Emoji("📊", "STACK");
}
