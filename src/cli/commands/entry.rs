use crate::errors::{CascadeError, Result};
use crate::stack::StackManager;
use clap::Subcommand;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::env;
use std::io;
use tracing::{info, warn};

#[derive(Debug, Subcommand)]
pub enum EntryAction {
    /// Interactively checkout a stack entry for editing
    Checkout {
        /// Stack entry number (optional, shows picker if not provided)
        #[arg(short, long)]
        entry: Option<usize>,
        /// Skip interactive picker and use entry number directly
        #[arg(long)]
        direct: bool,
        /// Skip confirmation prompts
        #[arg(long, short)]
        yes: bool,
    },
    /// Show current edit mode status
    Status {
        /// Show brief status only
        #[arg(long)]
        quiet: bool,
    },
    /// List all entries with their edit status
    List {
        /// Show detailed information
        #[arg(long, short)]
        verbose: bool,
    },
}

pub async fn run(action: EntryAction) -> Result<()> {
    let _current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    match action {
        EntryAction::Checkout { entry, direct, yes } => checkout_entry(entry, direct, yes).await,
        EntryAction::Status { quiet } => show_edit_status(quiet).await,
        EntryAction::List { verbose } => list_entries(verbose).await,
    }
}

/// Checkout a specific stack entry for editing
async fn checkout_entry(
    entry_num: Option<usize>,
    direct: bool,
    skip_confirmation: bool,
) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let mut manager = StackManager::new(&current_dir)?;

    // Get active stack
    let active_stack = manager.get_active_stack().ok_or_else(|| {
        CascadeError::config("No active stack. Create a stack first with 'cc stack create'")
    })?;

    if active_stack.entries.is_empty() {
        return Err(CascadeError::config(
            "Stack is empty. Push some commits first with 'cc stack push'",
        ));
    }

    // Determine which entry to checkout
    let target_entry_num = if let Some(num) = entry_num {
        if num == 0 || num > active_stack.entries.len() {
            return Err(CascadeError::config(format!(
                "Invalid entry number: {}. Stack has {} entries",
                num,
                active_stack.entries.len()
            )));
        }
        num
    } else if direct {
        return Err(CascadeError::config(
            "Entry number required when using --direct flag",
        ));
    } else {
        // Show interactive picker
        show_entry_picker(active_stack).await?
    };

    let target_entry = &active_stack.entries[target_entry_num - 1]; // Convert to 0-based index

    // Clone the values we need before borrowing manager mutably
    let stack_id = active_stack.id;
    let entry_id = target_entry.id;
    let entry_commit_hash = target_entry.commit_hash.clone();
    let entry_branch = target_entry.branch.clone();
    let entry_short_hash = target_entry.short_hash();
    let entry_short_message = target_entry.short_message(50);
    let entry_pr_id = target_entry.pull_request_id.clone();

    // Check if already in edit mode
    if manager.is_in_edit_mode() {
        let edit_info = manager.get_edit_mode_info().unwrap();
        warn!("Already in edit mode for entry in stack");

        if !skip_confirmation {
            println!("⚠️  Already in edit mode!");
            println!(
                "   Current target: {} (TODO: get commit message)",
                &edit_info.original_commit_hash[..8]
            );
            println!("   Do you want to exit current edit mode and start a new one? [y/N]");

            // TODO: Implement interactive confirmation
            // For now, just warn and exit
            return Err(CascadeError::config("Exit current edit mode first with 'cc entry status' and handle any pending changes"));
        }
    }

    // Confirmation prompt
    if !skip_confirmation {
        println!("🎯 Checking out entry for editing:");
        println!("   Entry #{target_entry_num}: {entry_short_hash} ({entry_short_message})");
        println!("   Branch: {entry_branch}");
        if let Some(pr_id) = &entry_pr_id {
            println!("   PR: #{pr_id}");
        }
        println!("\n⚠️  This will checkout the commit and enter edit mode.");
        println!("   Any changes you make can be amended to this commit or create new entries.");
        println!("\nContinue? [y/N]");

        // TODO: Implement interactive confirmation with dialoguer
        // For now, just proceed
        info!("Skipping confirmation for now - will implement interactive prompt in next step");
    }

    // Enter edit mode
    manager.enter_edit_mode(stack_id, entry_id)?;

    // Checkout the commit
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;
    let repo = crate::git::GitRepository::open(&current_dir)?;

    info!("Checking out commit: {}", entry_commit_hash);
    repo.checkout_commit(&entry_commit_hash)?;

    println!("✅ Entered edit mode for entry #{target_entry_num}");
    println!("   You are now on commit: {entry_short_hash} ({entry_short_message})");
    println!("   Branch: {entry_branch}");
    println!("\n📝 Make your changes and commit normally.");
    println!("   • Use 'cc entry status' to see edit mode info");
    println!("   • Changes will be smartly handled when you commit");
    println!("   • Use 'cc stack commit-edit' when ready (coming in next step)");

    Ok(())
}

/// Interactive entry picker using TUI
async fn show_entry_picker(stack: &crate::stack::Stack) -> Result<usize> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut list_state = ListState::default();
    list_state.select(Some(0));

    let result = loop {
        terminal.draw(|f| {
            let size = f.size();

            // Create layout
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints(
                    [
                        Constraint::Length(3), // Title
                        Constraint::Min(5),    // List
                        Constraint::Length(3), // Help
                    ]
                    .as_ref(),
                )
                .split(size);

            // Title
            let title = Paragraph::new(format!("📚 Select Entry from Stack: {}", stack.name))
                .style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(title, chunks[0]);

            // Entry list
            let items: Vec<ListItem> = stack
                .entries
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let status_icon = if entry.is_submitted {
                        if entry.pull_request_id.is_some() {
                            "📤"
                        } else {
                            "📝"
                        }
                    } else {
                        "🔄"
                    };

                    let pr_text = if let Some(pr_id) = &entry.pull_request_id {
                        format!(" PR: #{pr_id}")
                    } else {
                        "".to_string()
                    };

                    let line = Line::from(vec![
                        Span::raw(format!("  {}. ", i + 1)),
                        Span::raw(status_icon),
                        Span::raw(" "),
                        Span::styled(entry.short_message(40), Style::default().fg(Color::White)),
                        Span::raw(" "),
                        Span::styled(
                            format!("({})", entry.short_hash()),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(pr_text, Style::default().fg(Color::Green)),
                    ]);

                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Entries"))
                .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
                .highlight_symbol("→ ");

            f.render_stateful_widget(list, chunks[1], &mut list_state);

            // Help text
            let help = Paragraph::new("↑/↓: Navigate • Enter: Select • q: Quit • r: Refresh")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(help, chunks[2]);
        })?;

        // Handle input
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => {
                        break Err(CascadeError::config("Entry selection cancelled"));
                    }
                    KeyCode::Up => {
                        let selected = list_state.selected().unwrap_or(0);
                        if selected > 0 {
                            list_state.select(Some(selected - 1));
                        } else {
                            list_state.select(Some(stack.entries.len() - 1));
                        }
                    }
                    KeyCode::Down => {
                        let selected = list_state.selected().unwrap_or(0);
                        if selected < stack.entries.len() - 1 {
                            list_state.select(Some(selected + 1));
                        } else {
                            list_state.select(Some(0));
                        }
                    }
                    KeyCode::Enter => {
                        let selected = list_state.selected().unwrap_or(0);
                        break Ok(selected + 1); // Convert to 1-based index
                    }
                    KeyCode::Char('r') => {
                        // Refresh - for now just continue the loop
                        continue;
                    }
                    _ => {}
                }
            }
        }
    };

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Show current edit mode status
async fn show_edit_status(quiet: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;
    let manager = StackManager::new(&current_dir)?;

    if !manager.is_in_edit_mode() {
        if quiet {
            println!("inactive");
        } else {
            println!("📝 Not in edit mode");
            println!("   Use 'cc entry checkout' to start editing a stack entry");
        }
        return Ok(());
    }

    let edit_info = manager.get_edit_mode_info().unwrap();

    if quiet {
        println!("active:{:?}", edit_info.target_entry_id);
        return Ok(());
    }

    println!("🎯 Currently in edit mode");
    println!("   Target entry: {:?}", edit_info.target_entry_id);
    println!(
        "   Original commit: {}",
        &edit_info.original_commit_hash[..8]
    );
    println!(
        "   Started: {}",
        edit_info.started_at.format("%Y-%m-%d %H:%M:%S")
    );

    // Show current Git status
    println!("\n📋 Current state:");

    // TODO: Add Git status information
    // - Current HEAD vs original commit
    // - Working directory status
    // - Staged changes

    println!("   Use 'git status' for detailed working directory status");
    println!("   Use 'cc entry list' to see all entries");

    Ok(())
}

/// List all entries in the stack with edit status
async fn list_entries(verbose: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;
    let manager = StackManager::new(&current_dir)?;

    let active_stack = manager.get_active_stack().ok_or_else(|| {
        CascadeError::config(
            "No active stack. Create a stack first with 'cc stack create'".to_string(),
        )
    })?;

    if active_stack.entries.is_empty() {
        println!("📭 Active stack '{}' has no entries yet", active_stack.name);
        println!("   Add some commits to the stack with 'cc stack push'");
        return Ok(());
    }

    println!(
        "📚 Stack: {} ({} entries)",
        active_stack.name,
        active_stack.entries.len()
    );

    let edit_mode_info = manager.get_edit_mode_info();

    for (i, entry) in active_stack.entries.iter().enumerate() {
        let entry_num = i + 1;

        // Status icon
        let status_icon = if entry.is_submitted {
            if entry.pull_request_id.is_some() {
                "📤"
            } else {
                "📝"
            }
        } else {
            "🔄"
        };

        // Edit mode indicator
        let edit_indicator = if edit_mode_info.is_some()
            && edit_mode_info.unwrap().target_entry_id == Some(entry.id)
        {
            " 🎯"
        } else {
            ""
        };

        // Basic entry line
        print!(
            "   {}. {} {} ({})",
            entry_num,
            status_icon,
            entry.short_message(50),
            entry.short_hash()
        );

        // PR information
        if let Some(pr_id) = &entry.pull_request_id {
            print!(" PR: #{pr_id}");
        }

        print!("{edit_indicator}");
        println!();

        // Verbose information
        if verbose {
            println!("      Branch: {}", entry.branch);
            println!(
                "      Created: {}",
                entry.created_at.format("%Y-%m-%d %H:%M:%S")
            );
            if entry.is_submitted {
                println!("      Status: Submitted");
            } else {
                println!("      Status: Draft");
            }
            println!();
        }
    }

    if let Some(_edit_info) = edit_mode_info {
        println!("\n🎯 Edit mode active - use 'cc entry status' for details");
    } else {
        println!("\n💡 Use 'cc entry checkout' to start editing an entry");
    }

    Ok(())
}
