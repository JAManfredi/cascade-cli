use console::{style, Color, Emoji, Style};
use std::fmt::Display;

/// Theme configuration for Cascade CLI
/// Matches the branding: black, gray, green palette
struct Theme;

impl Theme {
    /// Bright/bold green for success messages (matches banner accent)
    /// Using Color256(46) for a brighter, more vibrant green
    fn success_style() -> Style {
        Style::new().color256(46).bold() // Bright bold green
    }

    /// Red for errors
    const ERROR: Color = Color::Red;

    /// Yellow for warnings
    const WARNING: Color = Color::Yellow;

    /// Muted green (Color256) for info - complements success green
    /// Using terminal color 35 (teal/green) for better readability
    fn info_style() -> Style {
        Style::new().color256(35) // Muted teal-green
    }

    /// Same muted green for tips
    fn tip_style() -> Style {
        Style::new().color256(35) // Muted teal-green
    }

    /// Dim gray for secondary text
    fn dim_style() -> Style {
        Style::new().dim()
    }
}

/// Centralized output formatting utilities for consistent CLI presentation
pub struct Output;

impl Output {
    /// Print a success message with checkmark (bright bold green)
    pub fn success<T: Display>(message: T) {
        println!("{} {}", Theme::success_style().apply_to("✓"), message);
    }

    /// Print an error message with X mark
    pub fn error<T: Display>(message: T) {
        println!("{} {}", style("✗").fg(Theme::ERROR), message);
    }

    /// Print a warning message with warning emoji
    pub fn warning<T: Display>(message: T) {
        println!("{} {}", style("⚠").fg(Theme::WARNING), message);
    }

    /// Print an info message with info emoji (muted green)
    pub fn info<T: Display>(message: T) {
        println!("{} {}", Theme::info_style().apply_to("ℹ"), message);
    }

    /// Print a sub-item with arrow prefix
    pub fn sub_item<T: Display>(message: T) {
        println!("  {} {}", Theme::dim_style().apply_to("→"), message);
    }

    /// Print a bullet point
    pub fn bullet<T: Display>(message: T) {
        println!("  {} {}", Theme::dim_style().apply_to("•"), message);
    }

    /// Print a section header
    pub fn section<T: Display>(title: T) {
        println!("\n{}", style(title).bold().underlined());
    }

    /// Print a tip/suggestion (muted green)
    pub fn tip<T: Display>(message: T) {
        println!(
            "{} {}",
            Theme::tip_style().apply_to("TIP:"),
            Theme::dim_style().apply_to(message)
        );
    }

    /// Print progress indicator (muted green)
    pub fn progress<T: Display>(message: T) {
        print!("{} {}", Theme::info_style().apply_to("→"), message);
        use std::io::{self, Write};
        io::stdout().flush().unwrap();
    }

    /// Print success checkmark inline (for use after progress)
    pub fn success_inline() {
        println!(" {}", Theme::success_style().apply_to("✓"));
    }

    /// Print error cross inline (for use after progress)
    pub fn error_inline<T: Display>(message: T) {
        if message.to_string().is_empty() {
            println!(" {}", style("✗").fg(Theme::ERROR));
        } else {
            println!(" {} {}", style("✗").fg(Theme::ERROR), message);
        }
    }

    /// Print a divider line
    pub fn divider() {
        println!("{}", Theme::dim_style().apply_to("─".repeat(50)));
    }

    /// Print stack information in a formatted way
    pub fn stack_info(
        name: &str,
        id: &str,
        base_branch: &str,
        working_branch: Option<&str>,
        is_active: bool,
    ) {
        // Show as info, not success (we're viewing, not creating)
        println!(
            "{} {}",
            Theme::info_style().apply_to("Stack:"),
            style(name).bold()
        );
        Self::sub_item(format!("Stack ID: {}", Theme::dim_style().apply_to(id)));
        Self::sub_item(format!(
            "Base branch: {}",
            Theme::info_style().apply_to(base_branch)
        ));

        if let Some(working) = working_branch {
            Self::sub_item(format!(
                "Working branch: {}",
                Theme::info_style().apply_to(working)
            ));
        }

        if is_active {
            Self::sub_item(format!(
                "Status: {}",
                Theme::success_style().apply_to("Active")
            ));
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
        println!("  {}", style(command).fg(Theme::WARNING));
    }

    /// Print a check start message
    pub fn check_start<T: Display>(message: T) {
        println!("\n{} {}", style("🔍").bright(), style(message).bold());
    }

    /// Print a solution message
    pub fn solution<T: Display>(message: T) {
        println!("     {}: {}", style("Solution").fg(Theme::WARNING), message);
    }

    /// Print a numbered item (muted green)
    pub fn numbered_item<T: Display>(number: usize, message: T) {
        println!("  {}. {}", Theme::info_style().apply_to(number), message);
    }

    /// Print empty line for spacing
    pub fn spacing() {
        println!();
    }

    /// Format stack entry status with appropriate color
    /// - pending: Yellow (work in progress)
    /// - submitted: Muted green (PR open/under review)
    /// - merged: Bright green (completed!)
    pub fn entry_status(is_submitted: bool, is_merged: bool) -> String {
        if is_merged {
            format!("{}", Theme::success_style().apply_to("[merged]"))
        } else if is_submitted {
            format!("{}", Theme::info_style().apply_to("[submitted]"))
        } else {
            format!("{}", style("[pending]").fg(Theme::WARNING))
        }
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
