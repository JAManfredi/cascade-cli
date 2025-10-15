use crate::cli::output::Output;
use crate::errors::{CascadeError, Result};
use crate::git::find_repository_root;
use crate::stack::StackManager;
use clap::Subcommand;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dialoguer::{theme::ColorfulTheme, Confirm};
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
use tracing::debug;

#[derive(Debug, Subcommand)]
pub enum EntryAction {
    /// Interactively checkout a stack entry for editing
    Checkout {
        /// Stack entry number (optional, shows picker if not provided)
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
    /// Clear/exit edit mode (useful for recovering from corrupted state)
    Clear {
        /// Skip confirmation prompt
        #[arg(long, short)]
        yes: bool,
    },
    /// Amend the current stack entry commit and update working branch
    Amend {
        /// New commit message (optional, uses git editor if not provided)
        #[arg(long, short)]
        message: Option<String>,
        /// Include all changes (staged + unstaged) like 'git commit -a --amend'
        #[arg(long, short)]
        all: bool,
        /// Automatically force-push after amending (if PR exists)
        #[arg(long)]
        push: bool,
        /// Auto-restack dependent entries after amending
        #[arg(long)]
        restack: bool,
    },
}

pub async fn run(action: EntryAction) -> Result<()> {
    let _current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    match action {
        EntryAction::Checkout { entry, direct, yes } => checkout_entry(entry, direct, yes).await,
        EntryAction::Status { quiet } => show_edit_status(quiet).await,
        EntryAction::List { verbose } => list_entries(verbose).await,
        EntryAction::Clear { yes } => clear_edit_mode(yes).await,
        EntryAction::Amend {
            message,
            all,
            push,
            restack,
        } => amend_entry(message, all, push, restack).await,
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

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut manager = StackManager::new(&repo_root)?;

    // Get active stack
    let active_stack = manager.get_active_stack().ok_or_else(|| {
        CascadeError::config("No active stack. Create a stack first with 'ca stack create'")
    })?;

    if active_stack.entries.is_empty() {
        return Err(CascadeError::config(
            "Stack is empty. Push some commits first with 'ca stack push'",
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
    let entry_branch = target_entry.branch.clone();
    let entry_short_hash = target_entry.short_hash();
    let entry_short_message = target_entry.short_message(50);
    let entry_pr_id = target_entry.pull_request_id.clone();
    let entry_message = target_entry.message.clone();

    // Check if already in edit mode and get info before confirmation
    let already_in_edit_mode = manager.is_in_edit_mode();
    let edit_mode_display = if already_in_edit_mode {
        let edit_info = manager.get_edit_mode_info().unwrap();

        // Get the commit message for the current edit target
        let commit_message = if let Some(target_entry_id) = &edit_info.target_entry_id {
            if let Some(entry) = active_stack
                .entries
                .iter()
                .find(|e| e.id == *target_entry_id)
            {
                entry.short_message(50)
            } else {
                "Unknown entry".to_string()
            }
        } else {
            "Unknown target".to_string()
        };

        Some((edit_info.original_commit_hash.clone(), commit_message))
    } else {
        None
    };

    // Let the active_stack reference go out of scope before we potentially mutably borrow manager
    let _ = active_stack;

    // Handle edit mode exit if needed
    if let Some((commit_hash, commit_message)) = edit_mode_display {
        tracing::debug!("Already in edit mode for entry in stack");

        if !skip_confirmation {
            Output::warning("Already in edit mode!");
            Output::sub_item(format!(
                "Current target: {} ({})",
                &commit_hash[..8],
                commit_message
            ));

            // Interactive confirmation to exit current edit mode
            let should_exit_edit_mode = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Exit current edit mode and start a new one?")
                .default(false)
                .interact()
                .map_err(|e| {
                    CascadeError::config(format!("Failed to get user confirmation: {e}"))
                })?;

            if !should_exit_edit_mode {
                return Err(CascadeError::config(
                    "Operation cancelled. Use 'ca entry status' to see current edit mode details.",
                ));
            }

            // Exit current edit mode before starting a new one
            Output::info("Exiting current edit mode...");
            manager.exit_edit_mode()?;
            Output::success("✓ Exited previous edit mode");
        }
    }

    // Confirmation prompt
    if !skip_confirmation {
        Output::section("Checking out entry for editing");
        Output::sub_item(format!(
            "Entry #{target_entry_num}: {entry_short_hash} ({entry_short_message})"
        ));
        Output::sub_item(format!("Branch: {entry_branch}"));
        if let Some(pr_id) = &entry_pr_id {
            Output::sub_item(format!("PR: #{pr_id}"));
        }

        // Display full commit message
        Output::sub_item("Commit Message:");
        let lines: Vec<&str> = entry_message.lines().collect();
        for line in lines {
            Output::sub_item(format!("  {line}"));
        }

        Output::warning("This will checkout the commit and enter edit mode.");
        Output::info("Any changes you make can be amended to this commit or create new entries.");

        // Interactive confirmation to proceed with checkout
        let should_continue = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Continue with checkout?")
            .default(false)
            .interact()
            .map_err(|e| CascadeError::config(format!("Failed to get user confirmation: {e}")))?;

        if !should_continue {
            return Err(CascadeError::config("Entry checkout cancelled"));
        }
    }

    // Enter edit mode
    manager.enter_edit_mode(stack_id, entry_id)?;

    // Checkout the branch (not the commit - we want to stay on the branch)
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;
    let repo = crate::git::GitRepository::open(&repo_root)?;

    debug!("Checking out branch: {}", entry_branch);
    repo.checkout_branch(&entry_branch)?;

    Output::success(format!("Entered edit mode for entry #{target_entry_num}"));
    Output::sub_item(format!(
        "You are now on commit: {} ({})",
        entry_short_hash, entry_short_message
    ));
    Output::sub_item(format!("Branch: {entry_branch}"));

    Output::section("Make your changes and commit normally");
    Output::bullet("Use 'ca entry status' to see edit mode info");
    Output::bullet("When you commit, the pre-commit hook will guide you");

    // Check if prepare-commit-msg hook is installed
    let hooks_dir = repo_root.join(".git/hooks");
    let hook_path = hooks_dir.join("prepare-commit-msg");
    if !hook_path.exists() {
        Output::tip("Install the prepare-commit-msg hook for better guidance:");
        Output::sub_item("ca hooks add prepare-commit-msg");
    }

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
            let size = f.area();

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

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;
    let manager = StackManager::new(&repo_root)?;

    if !manager.is_in_edit_mode() {
        if quiet {
            println!("inactive");
        } else {
            Output::info("Not in edit mode");
            Output::sub_item("Use 'ca entry checkout' to start editing a stack entry");
        }
        return Ok(());
    }

    let edit_info = manager.get_edit_mode_info().unwrap();

    if quiet {
        println!("active:{:?}", edit_info.target_entry_id);
        return Ok(());
    }

    Output::section("Currently in edit mode");

    // Try to get the entry information
    if let Some(active_stack) = manager.get_active_stack() {
        if let Some(target_entry_id) = edit_info.target_entry_id {
            if let Some(entry) = active_stack
                .entries
                .iter()
                .find(|e| e.id == target_entry_id)
            {
                Output::sub_item(format!(
                    "Target entry: {} ({})",
                    entry.short_hash(),
                    entry.short_message(50)
                ));
                Output::sub_item(format!("Branch: {}", entry.branch));

                // Display full commit message
                Output::sub_item("Commit Message:");
                let lines: Vec<&str> = entry.message.lines().collect();
                for line in lines {
                    Output::sub_item(format!("  {line}"));
                }
            } else {
                Output::sub_item(format!("Target entry: {target_entry_id:?} (not found)"));
            }
        } else {
            Output::sub_item("Target entry: Unknown");
        }
    } else {
        Output::sub_item(format!("Target entry: {:?}", edit_info.target_entry_id));
    }

    Output::sub_item(format!(
        "Original commit: {}",
        &edit_info.original_commit_hash[..8]
    ));
    Output::sub_item(format!(
        "Started: {}",
        edit_info.started_at.format("%Y-%m-%d %H:%M:%S")
    ));

    // Show current Git status
    Output::section("Current state");

    // Get current repository state
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;
    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;
    let repo = crate::git::GitRepository::open(&repo_root)?;

    // Current HEAD vs original commit
    let current_head = repo.get_current_commit_hash()?;
    if current_head != edit_info.original_commit_hash {
        let current_short = &current_head[..8];
        let original_short = &edit_info.original_commit_hash[..8];
        Output::sub_item(format!("HEAD moved: {original_short} → {current_short}"));

        // Show if there are new commits
        match repo.get_commit_count_between(&edit_info.original_commit_hash, &current_head) {
            Ok(count) if count > 0 => {
                Output::sub_item(format!("  {count} new commit(s) created"));
            }
            _ => {}
        }
    } else {
        Output::sub_item(format!("HEAD: {} (unchanged)", &current_head[..8]));
    }

    // Working directory and staging status
    match repo.get_status_summary() {
        Ok(status) => {
            if status.is_clean() {
                Output::sub_item("Working directory: clean");
            } else {
                if status.has_staged_changes() {
                    Output::sub_item(format!("Staged changes: {} files", status.staged_count()));
                }
                if status.has_unstaged_changes() {
                    Output::sub_item(format!(
                        "Unstaged changes: {} files",
                        status.unstaged_count()
                    ));
                }
                if status.has_untracked_files() {
                    Output::sub_item(format!(
                        "Untracked files: {} files",
                        status.untracked_count()
                    ));
                }
            }
        }
        Err(_) => {
            Output::sub_item("Working directory: status unavailable");
        }
    }

    Output::tip("Use 'git status' for detailed file-level status");
    Output::sub_item("Use 'ca entry list' to see all entries");

    Ok(())
}

/// List all entries in the stack with edit status
async fn list_entries(verbose: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;
    let manager = StackManager::new(&repo_root)?;

    let active_stack = manager.get_active_stack().ok_or_else(|| {
        CascadeError::config(
            "No active stack. Create a stack first with 'ca stack create'".to_string(),
        )
    })?;

    if active_stack.entries.is_empty() {
        Output::info(format!(
            "Active stack '{}' has no entries yet",
            active_stack.name
        ));
        Output::sub_item("Add some commits to the stack with 'ca stack push'");
        return Ok(());
    }

    Output::section(format!(
        "Stack: {} ({} entries)",
        active_stack.name,
        active_stack.entries.len()
    ));

    let edit_mode_info = manager.get_edit_mode_info();
    let edit_target_entry_id = edit_mode_info
        .as_ref()
        .and_then(|info| info.target_entry_id);

    for (i, entry) in active_stack.entries.iter().enumerate() {
        let entry_num = i + 1;
        let status_label = Output::entry_status(entry.is_submitted, entry.is_merged);
        let mut entry_line = format!(
            "{} {} ({})",
            status_label,
            entry.short_message(50),
            entry.short_hash()
        );

        if let Some(pr_id) = &entry.pull_request_id {
            entry_line.push_str(&format!(" PR: #{pr_id}"));
        }

        if Some(entry.id) == edit_target_entry_id {
            entry_line.push_str(" [edit target]");
        }

        Output::numbered_item(entry_num, entry_line);

        if verbose {
            Output::sub_item(format!("Branch: {}", entry.branch));
            Output::sub_item(format!("Commit: {}", entry.commit_hash));
            Output::sub_item(format!(
                "Created: {}",
                entry.created_at.format("%Y-%m-%d %H:%M:%S")
            ));

            if entry.is_merged {
                Output::sub_item("Status: Merged");
            } else if entry.is_submitted {
                Output::sub_item("Status: Submitted");
            } else {
                Output::sub_item("Status: Draft");
            }

            Output::sub_item("Message:");
            for line in entry.message.lines() {
                Output::sub_item(format!("  {line}"));
            }

            if Some(entry.id) == edit_target_entry_id {
                Output::sub_item("Edit mode target");

                match crate::git::GitRepository::open(&repo_root) {
                    Ok(repo) => match repo.get_status_summary() {
                        Ok(status) => {
                            if !status.is_clean() {
                                Output::sub_item("Git Status:");
                                if status.has_staged_changes() {
                                    Output::sub_item(format!(
                                        "  Staged: {} files",
                                        status.staged_count()
                                    ));
                                }
                                if status.has_unstaged_changes() {
                                    Output::sub_item(format!(
                                        "  Unstaged: {} files",
                                        status.unstaged_count()
                                    ));
                                }
                                if status.has_untracked_files() {
                                    Output::sub_item(format!(
                                        "  Untracked: {} files",
                                        status.untracked_count()
                                    ));
                                }
                            } else {
                                Output::sub_item("Git Status: clean");
                            }
                        }
                        Err(_) => {
                            Output::sub_item("Git Status: unavailable");
                        }
                    },
                    Err(_) => {
                        Output::sub_item("Git Status: unavailable");
                    }
                }
            }
        }
    }

    if edit_mode_info.is_some() {
        Output::spacing();
        Output::info("Edit mode active - use 'ca entry status' for details");
    } else {
        Output::spacing();
        Output::tip("Use 'ca entry checkout' to start editing an entry");
    }

    Ok(())
}

/// Clear/exit edit mode (useful for recovering from corrupted state)
async fn clear_edit_mode(skip_confirmation: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut manager = StackManager::new(&repo_root)?;

    if !manager.is_in_edit_mode() {
        Output::info("Not currently in edit mode");
        return Ok(());
    }

    // Show current edit mode info
    if let Some(edit_info) = manager.get_edit_mode_info() {
        Output::section("Current edit mode state");

        if let Some(target_entry_id) = &edit_info.target_entry_id {
            Output::sub_item(format!("Target entry: {}", target_entry_id));

            // Try to find the entry
            if let Some(active_stack) = manager.get_active_stack() {
                if let Some(entry) = active_stack
                    .entries
                    .iter()
                    .find(|e| e.id == *target_entry_id)
                {
                    Output::sub_item(format!("Entry: {}", entry.short_message(50)));
                } else {
                    Output::warning("Target entry not found in stack (corrupted state)");
                }
            }
        }

        Output::sub_item(format!(
            "Original commit: {}",
            &edit_info.original_commit_hash[..8]
        ));
        Output::sub_item(format!(
            "Started: {}",
            edit_info.started_at.format("%Y-%m-%d %H:%M:%S")
        ));
    }

    // Confirm before clearing
    if !skip_confirmation {
        println!();
        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Clear edit mode state?")
            .default(true)
            .interact()
            .map_err(|e| CascadeError::config(format!("Failed to get user confirmation: {e}")))?;

        if !confirmed {
            return Err(CascadeError::config("Operation cancelled."));
        }
    }

    // Clear edit mode
    manager.exit_edit_mode()?;

    Output::success("Edit mode cleared");
    Output::tip("Use 'ca entry checkout' to start a new edit session");

    Ok(())
}

/// Amend the current stack entry commit and update working branch
async fn amend_entry(message: Option<String>, all: bool, push: bool, restack: bool) -> Result<()> {
    let current_dir = env::current_dir()
        .map_err(|e| CascadeError::config(format!("Could not get current directory: {e}")))?;

    let repo_root = find_repository_root(&current_dir)
        .map_err(|e| CascadeError::config(format!("Could not find git repository: {e}")))?;

    let mut manager = StackManager::new(&repo_root)?;
    let repo = crate::git::GitRepository::open(&repo_root)?;

    let current_branch = repo.get_current_branch()?;

    // Get active stack info we need (clone to avoid borrow issues)
    let (stack_id, entry_index, entry_id, entry_branch, working_branch, has_dependents, has_pr) = {
        let active_stack = manager.get_active_stack().ok_or_else(|| {
            CascadeError::config("No active stack. Create a stack first with 'ca stack create'")
        })?;

        // Find which entry we're amending (must be on a stack branch)
        let mut found_entry = None;

        for (idx, entry) in active_stack.entries.iter().enumerate() {
            if entry.branch == current_branch {
                found_entry = Some((
                    idx,
                    entry.id,
                    entry.branch.clone(),
                    entry.pull_request_id.clone(),
                ));
                break;
            }
        }

        match found_entry {
            Some((idx, id, branch, pr_id)) => {
                let has_dependents = idx + 1 < active_stack.entries.len();
                (
                    active_stack.id,
                    idx,
                    id,
                    branch,
                    active_stack.working_branch.clone(),
                    has_dependents,
                    pr_id.is_some(),
                )
            }
            None => {
                return Err(CascadeError::config(format!(
                    "Current branch '{}' is not a stack entry branch.\n\
                     Use 'ca entry checkout <N>' to checkout a stack entry first.",
                    current_branch
                )));
            }
        }
    };

    Output::section(format!("Amending stack entry #{}", entry_index + 1));

    // 1. Perform the git commit --amend
    let mut amend_args = vec!["commit", "--amend"];

    if all {
        amend_args.insert(1, "-a");
    }

    if let Some(ref msg) = message {
        amend_args.push("-m");
        amend_args.push(msg);
    } else {
        // Use git editor for interactive message editing
        amend_args.push("--no-edit");
    }

    debug!("Running git {}", amend_args.join(" "));

    // Set environment variable to bypass pre-commit hook (avoid infinite loop)
    let output = std::process::Command::new("git")
        .args(&amend_args)
        .env("CASCADE_SKIP_HOOKS", "1")
        .current_dir(&repo_root)
        .stdout(std::process::Stdio::null()) // Suppress Git's output
        .stderr(std::process::Stdio::piped()) // Capture errors
        .output()
        .map_err(CascadeError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CascadeError::branch(format!(
            "Failed to amend commit: {}",
            stderr.trim()
        )));
    }

    Output::success("Commit amended");

    // 2. Get the new commit hash
    let new_commit_hash = repo.get_head_commit()?.id().to_string();
    debug!("New commit hash after amend: {}", new_commit_hash);

    // 3. Update stack metadata with new commit hash using safe wrapper
    {
        let stack = manager
            .get_stack_mut(&stack_id)
            .ok_or_else(|| CascadeError::config("Stack not found"))?;

        let old_hash = stack
            .entries
            .iter()
            .find(|e| e.id == entry_id)
            .map(|e| e.commit_hash.clone())
            .ok_or_else(|| CascadeError::config("Entry not found"))?;

        stack
            .update_entry_commit_hash(&entry_id, new_commit_hash.clone())
            .map_err(CascadeError::config)?;

        debug!(
            "Updated entry commit hash: {} -> {}",
            &old_hash[..8],
            &new_commit_hash[..8]
        );
        Output::sub_item(format!(
            "Updated metadata: {} → {}",
            &old_hash[..8],
            &new_commit_hash[..8]
        ));
    }

    manager.save_to_disk()?;

    // 4. Update working branch to keep safety net in sync
    if let Some(ref working_branch_name) = working_branch {
        Output::sub_item(format!("Updating working branch: {}", working_branch_name));

        // Force update the working branch to point to the amended commit
        repo.update_branch_to_commit(working_branch_name, &new_commit_hash)?;

        Output::success(format!("Working branch '{}' updated", working_branch_name));
    } else {
        Output::warning("No working branch found - create one with 'ca stack create' for safety");
    }

    // 5. Auto-restack dependent entries if requested
    if restack && has_dependents {
        println!();
        Output::info("Auto-restacking dependent entries");

        // Create fresh instances for rebase manager
        let rebase_manager_stack = StackManager::new(&repo_root)?;
        let stack_for_count = rebase_manager_stack
            .get_stack(&stack_id)
            .ok_or_else(|| CascadeError::config("Stack not found"))?;

        // Count dependent entries (entries after the current one)
        let entry_index = stack_for_count
            .entries
            .iter()
            .position(|e| e.id == entry_id)
            .unwrap_or(0);
        let dependent_count = stack_for_count
            .entries
            .len()
            .saturating_sub(entry_index + 1);

        if dependent_count > 0 {
            let rebase_manager_repo = crate::git::GitRepository::open(&repo_root)?;
            let plural = if dependent_count == 1 {
                "entry"
            } else {
                "entries"
            };

            println!(); // Spacing
            let rebase_spinner = crate::utils::spinner::Spinner::new_with_output_below(format!(
                "Restacking {} dependent {}",
                dependent_count, plural
            ));

            // Use the sync_stack mechanism to rebase dependent entries
            let mut rebase_manager = crate::stack::RebaseManager::new(
                rebase_manager_stack,
                rebase_manager_repo,
                crate::stack::RebaseOptions {
                    strategy: crate::stack::RebaseStrategy::ForcePush,
                    target_base: Some(entry_branch.clone()),
                    skip_pull: Some(true), // Don't pull, we're rebasing on local changes
                    ..Default::default()
                },
            );

            let rebase_result = rebase_manager.rebase_stack(&stack_id);

            rebase_spinner.stop();
            println!(); // Spacing

            match rebase_result {
                Ok(_) => {
                    Output::success("Dependent entries restacked");
                }
                Err(e) => {
                    Output::warning(format!("Could not auto-restack: {}", e));
                    Output::tip("Run 'ca sync' manually to restack dependent entries");
                }
            }
        }
    }

    // 6. Auto-push if requested and entry has a PR
    if push {
        println!();

        if has_pr {
            Output::section("Force-pushing to remote");

            // Set env var to skip force-push confirmation
            std::env::set_var("FORCE_PUSH_NO_CONFIRM", "1");

            repo.force_push_branch(&current_branch, &current_branch)?;
            Output::success(format!("Force-pushed '{}' to remote", current_branch));
            Output::sub_item("PR will be automatically updated");
        } else {
            Output::warning("No PR found for this entry - skipping push");
            Output::tip("Use 'ca submit' to create a PR");
        }
    }

    // Summary
    println!();
    Output::section("Summary");
    Output::bullet(format!(
        "Amended entry #{} on branch '{}'",
        entry_index + 1,
        entry_branch
    ));
    if working_branch.is_some() {
        Output::bullet("Working branch updated");
    }
    if restack {
        Output::bullet("Dependent entries restacked");
    }
    if push {
        Output::bullet("Changes force-pushed to remote");
    }

    // Tips section (separated from summary)
    // Note: restack auto-pushes, so don't show push tip if restack is enabled
    if (has_dependents || !push) && !restack {
        println!();
        if !push && !restack {
            Output::tip("Use --push to automatically force-push after amending");
        }
        if !restack && has_dependents {
            Output::tip("Use --restack to automatically update dependent entries");
        }
    }

    Ok(())
}
