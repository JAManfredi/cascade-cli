use crate::errors::{CascadeError, Result};
use crate::stack::{StackManager, StackStatus};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, Tabs, Wrap,
    },
    Frame, Terminal,
};
use std::env;
use std::io;
use std::time::{Duration, Instant};

/// TUI Application state
pub struct TuiApp {
    should_quit: bool,
    stack_manager: StackManager,
    stacks: Vec<crate::stack::Stack>,
    selected_stack: usize,
    selected_tab: usize,
    stack_list_state: ListState,
    last_refresh: Instant,
    refresh_interval: Duration,
    show_help: bool,
    show_details: bool,
    error_message: Option<String>,
}

impl TuiApp {
    pub fn new() -> Result<Self> {
        let current_dir = env::current_dir()
            .map_err(|e| CascadeError::config(format!("Could not get current directory: {}", e)))?;

        let stack_manager = StackManager::new(&current_dir)?;
        let stacks = stack_manager.get_all_stacks_objects()?;

        let mut stack_list_state = ListState::default();
        if !stacks.is_empty() {
            stack_list_state.select(Some(0));
        }

        Ok(TuiApp {
            should_quit: false,
            stack_manager,
            stacks,
            selected_stack: 0,
            selected_tab: 0,
            stack_list_state,
            last_refresh: Instant::now(),
            refresh_interval: Duration::from_secs(10),
            show_help: false,
            show_details: false,
            error_message: None,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()
            .map_err(|e| CascadeError::config(format!("Failed to enable raw mode: {}", e)))?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| CascadeError::config(format!("Failed to setup terminal: {}", e)))?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)
            .map_err(|e| CascadeError::config(format!("Failed to create terminal: {}", e)))?;

        // Main loop
        let result = self.run_app(&mut terminal);

        // Restore terminal
        disable_raw_mode()
            .map_err(|e| CascadeError::config(format!("Failed to disable raw mode: {}", e)))?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .map_err(|e| CascadeError::config(format!("Failed to restore terminal: {}", e)))?;
        terminal
            .show_cursor()
            .map_err(|e| CascadeError::config(format!("Failed to show cursor: {}", e)))?;

        result
    }

    fn run_app<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal
                .draw(|f| self.draw(f))
                .map_err(|e| CascadeError::config(format!("Failed to draw: {}", e)))?;

            // Handle events with timeout for refresh
            let timeout = Duration::from_millis(100);
            if crossterm::event::poll(timeout)
                .map_err(|e| CascadeError::config(format!("Event poll failed: {}", e)))?
            {
                if let Event::Key(key) = event::read()
                    .map_err(|e| CascadeError::config(format!("Failed to read event: {}", e)))?
                {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key_event(key.code)?;
                    }
                }
            }

            // Auto-refresh data
            if self.last_refresh.elapsed() >= self.refresh_interval {
                self.refresh_data()?;
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyCode) -> Result<()> {
        if self.show_help {
            match key {
                KeyCode::Char('h') | KeyCode::Char('?') | KeyCode::Esc => {
                    self.show_help = false;
                }
                _ => {}
            }
            return Ok(());
        }

        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('h') | KeyCode::Char('?') => {
                self.show_help = true;
            }
            KeyCode::Char('r') => {
                self.refresh_data()?;
            }
            KeyCode::Char('d') => {
                self.show_details = !self.show_details;
            }
            KeyCode::Tab => {
                self.selected_tab = (self.selected_tab + 1) % 3; // 3 tabs: Stacks, Details, Actions
            }
            KeyCode::Up => {
                self.previous_stack();
            }
            KeyCode::Down => {
                self.next_stack();
            }
            KeyCode::Enter => {
                self.activate_selected_stack()?;
            }
            KeyCode::Char('c') => {
                // Create new stack (placeholder)
                self.error_message = Some("Create stack: Not implemented yet".to_string());
            }
            KeyCode::Char('s') => {
                // Submit selected entry (placeholder)
                self.error_message = Some("Submit entry: Not implemented yet".to_string());
            }
            KeyCode::Char('p') => {
                // Push to stack (placeholder)
                self.error_message = Some("Push to stack: Not implemented yet".to_string());
            }
            _ => {}
        }
        Ok(())
    }

    fn refresh_data(&mut self) -> Result<()> {
        self.stacks = self.stack_manager.get_all_stacks_objects()?;
        self.last_refresh = Instant::now();
        self.error_message = None;
        Ok(())
    }

    fn next_stack(&mut self) {
        if !self.stacks.is_empty() {
            let i = match self.stack_list_state.selected() {
                Some(i) => {
                    if i >= self.stacks.len() - 1 {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            self.stack_list_state.select(Some(i));
            self.selected_stack = i;
        }
    }

    fn previous_stack(&mut self) {
        if !self.stacks.is_empty() {
            let i = match self.stack_list_state.selected() {
                Some(i) => {
                    if i == 0 {
                        self.stacks.len() - 1
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            self.stack_list_state.select(Some(i));
            self.selected_stack = i;
        }
    }

    fn activate_selected_stack(&mut self) -> Result<()> {
        if let Some(stack) = self.stacks.get(self.selected_stack) {
            self.stack_manager.set_active_stack(Some(stack.id))?;
            self.error_message = Some(format!("Activated stack: {}", stack.name));
        }
        Ok(())
    }

    fn draw(&mut self, f: &mut Frame) {
        let size = f.size();

        // Main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Body
                Constraint::Length(3), // Footer
            ])
            .split(size);

        self.draw_header(f, chunks[0]);
        self.draw_body(f, chunks[1]);
        self.draw_footer(f, chunks[2]);

        // Overlays
        if self.show_help {
            self.draw_help_popup(f, size);
        }

        if let Some(ref msg) = self.error_message {
            self.draw_status_popup(f, size, msg);
        }
    }

    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let title = Paragraph::new("🌊 Cascade CLI - Interactive Stack Manager")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(title, area);
    }

    fn draw_body(&mut self, f: &mut Frame, area: Rect) {
        let tabs = vec!["📚 Stacks", "🔍 Details", "⚡ Actions"];
        let tab_titles = tabs.iter().cloned().map(Line::from).collect();
        let tabs_widget = Tabs::new(tab_titles)
            .block(Block::default().borders(Borders::ALL).title("Navigation"))
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .select(self.selected_tab);

        let body_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        f.render_widget(tabs_widget, body_chunks[0]);

        match self.selected_tab {
            0 => self.draw_stacks_tab(f, body_chunks[1]),
            1 => self.draw_details_tab(f, body_chunks[1]),
            2 => self.draw_actions_tab(f, body_chunks[1]),
            _ => {}
        }
    }

    fn draw_stacks_tab(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Stack list
        let items: Vec<ListItem> = self
            .stacks
            .iter()
            .enumerate()
            .map(|(i, stack)| {
                let status_icon = match stack.status {
                    StackStatus::Clean => "✅",
                    StackStatus::Dirty => "🔄",
                    StackStatus::OutOfSync => "⚠️",
                    StackStatus::Conflicted => "❌",
                    StackStatus::Rebasing => "🔀",
                    StackStatus::NeedsSync => "🔄",
                    StackStatus::Corrupted => "💥",
                };

                let active_marker = if stack.is_active { "👉 " } else { "   " };

                let content = format!(
                    "{}{} {} ({} entries)",
                    active_marker,
                    status_icon,
                    stack.name,
                    stack.entries.len()
                );

                let style = if i == self.selected_stack {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                ListItem::new(content).style(style)
            })
            .collect();

        let stacks_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("🗂️ Stacks"))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        f.render_stateful_widget(stacks_list, chunks[0], &mut self.stack_list_state);

        // Stack summary
        self.draw_stack_summary(f, chunks[1]);
    }

    fn draw_stack_summary(&self, f: &mut Frame, area: Rect) {
        if let Some(stack) = self.stacks.get(self.selected_stack) {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Name: ", Style::default().fg(Color::Cyan)),
                    Span::raw(&stack.name),
                ]),
                Line::from(vec![
                    Span::styled("Base: ", Style::default().fg(Color::Cyan)),
                    Span::raw(&stack.base_branch),
                ]),
                Line::from(vec![
                    Span::styled("Entries: ", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("{}", stack.entries.len())),
                ]),
                Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::Cyan)),
                    Span::raw(format!("{:?}", stack.status)),
                ]),
                Line::from(""),
            ];

            if let Some(desc) = &stack.description {
                lines.push(Line::from(vec![Span::styled(
                    "Description: ",
                    Style::default().fg(Color::Cyan),
                )]));
                lines.push(Line::from(desc.clone()));
                lines.push(Line::from(""));
            }

            // Recent entries
            if !stack.entries.is_empty() {
                lines.push(Line::from(vec![Span::styled(
                    "Recent Commits:",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )]));

                for (i, entry) in stack.entries.iter().rev().take(5).enumerate() {
                    lines.push(Line::from(format!(
                        "  {} {} - {}",
                        i + 1,
                        entry.short_hash(),
                        entry.short_message(40)
                    )));
                }
            }

            let summary = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("📊 Stack Info"),
                )
                .wrap(Wrap { trim: true });

            f.render_widget(summary, area);
        } else {
            let empty = Paragraph::new("No stacks available.\n\nPress 'c' to create a new stack.")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("📊 Stack Info"),
                )
                .alignment(Alignment::Center);
            f.render_widget(empty, area);
        }
    }

    fn draw_details_tab(&self, f: &mut Frame, area: Rect) {
        if let Some(stack) = self.stacks.get(self.selected_stack) {
            if stack.entries.is_empty() {
                let empty = Paragraph::new(
                    "No commits in this stack.\n\nUse 'cc stack push' to add commits.",
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("📋 Stack Details"),
                )
                .alignment(Alignment::Center);
                f.render_widget(empty, area);
                return;
            }

            let header = vec!["#", "Commit", "Branch", "Message", "Status"];
            let rows = stack.entries.iter().enumerate().map(|(i, entry)| {
                let status = if entry.pull_request_id.is_some() {
                    "📤 Submitted"
                } else {
                    "⏳ Pending"
                };

                Row::new(vec![
                    Cell::from((i + 1).to_string()),
                    Cell::from(entry.short_hash()),
                    Cell::from(entry.branch.clone()),
                    Cell::from(entry.short_message(30)),
                    Cell::from(status),
                ])
            });

            let table = Table::new(
                rows,
                [
                    Constraint::Length(3),
                    Constraint::Length(8),
                    Constraint::Length(20),
                    Constraint::Length(35),
                    Constraint::Length(12),
                ],
            )
            .header(
                Row::new(header)
                    .style(
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                    .bottom_margin(1),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("📋 Stack Details"),
            );

            f.render_widget(table, area);
        } else {
            let empty = Paragraph::new("No stack selected")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("📋 Stack Details"),
                )
                .alignment(Alignment::Center);
            f.render_widget(empty, area);
        }
    }

    fn draw_actions_tab(&self, f: &mut Frame, area: Rect) {
        let actions = vec![
            "📌 Enter - Activate selected stack",
            "📝 c - Create new stack",
            "🚀 p - Push current commit to stack",
            "📤 s - Submit entry for review",
            "🔄 r - Refresh data",
            "🔍 d - Toggle details view",
            "❓ h/? - Show help",
            "🚪 q/Esc - Quit",
        ];

        let lines: Vec<Line> = actions.iter().map(|&action| Line::from(action)).collect();

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("⚡ Quick Actions"),
            )
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, area);
    }

    fn draw_footer(&self, f: &mut Frame, area: Rect) {
        let last_refresh = format!("Last refresh: {:?} ago", self.last_refresh.elapsed());
        let key_hints = " h:Help │ q:Quit │ r:Refresh │ Tab:Navigate │ ↑↓:Select │ Enter:Activate ";

        let footer_text = format!("{} │ {}", last_refresh, key_hints);

        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(footer, area);
    }

    fn draw_help_popup(&self, f: &mut Frame, area: Rect) {
        let popup_area = self.centered_rect(80, 70, area);

        let help_text = vec![
            Line::from(vec![Span::styled(
                "🌊 Cascade CLI - Interactive Stack Manager",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "📍 Navigation:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("  ↑↓ - Navigate stacks"),
            Line::from("  Tab - Switch between tabs"),
            Line::from("  Enter - Activate selected stack"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "⚡ Actions:",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("  c - Create new stack"),
            Line::from("  p - Push commit to active stack"),
            Line::from("  s - Submit entry for review"),
            Line::from("  r - Refresh data"),
            Line::from("  d - Toggle details view"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "🎛️ Controls:",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("  h/? - Show this help"),
            Line::from("  q/Esc - Quit"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "💡 Tips:",
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from("  • Data refreshes automatically every 10 seconds"),
            Line::from("  • Use CLI commands for complex operations"),
            Line::from("  • Active stack is marked with 👉"),
            Line::from(""),
            Line::from("Press any key to close this help..."),
        ];

        let help_paragraph = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("❓ Help")
                    .style(Style::default().fg(Color::White)),
            )
            .wrap(Wrap { trim: true });

        f.render_widget(Clear, popup_area);
        f.render_widget(help_paragraph, popup_area);
    }

    fn draw_status_popup(&self, f: &mut Frame, area: Rect, message: &str) {
        let popup_area = self.centered_rect(60, 20, area);

        let status_paragraph = Paragraph::new(message)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("💬 Status")
                    .style(Style::default().fg(Color::Yellow)),
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        f.render_widget(Clear, popup_area);
        f.render_widget(status_paragraph, popup_area);
    }

    fn centered_rect(&self, percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}

/// Run the TUI application
pub async fn run() -> Result<()> {
    let mut app = TuiApp::new()?;
    app.run()
}
