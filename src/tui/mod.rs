//! TUI module - Terminal UI for interactive visualization

use crate::{Result, state_machine::StateGraph};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

pub mod app;
pub mod ui;

use app::{App, ViewMode};

/// Run the TUI application
pub fn run(
    graph: StateGraph,
    transactions: Vec<crate::data_source::Transaction>,
    update_receiver: Option<mpsc::Receiver<(StateGraph, Vec<crate::data_source::Transaction>)>>,
) -> Result<()> {
    // Setup terminal
    enable_raw_mode().map_err(|e| crate::Error::Tui(e.to_string()))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| crate::Error::Tui(e.to_string()))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| crate::Error::Tui(e.to_string()))?;

    // Create app and run
    let app = App::new(graph, transactions);
    let res = run_app(&mut terminal, app, update_receiver);

    // Restore terminal
    disable_raw_mode().map_err(|e| crate::Error::Tui(e.to_string()))?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .map_err(|e| crate::Error::Tui(e.to_string()))?;
    terminal
        .show_cursor()
        .map_err(|e| crate::Error::Tui(e.to_string()))?;

    res
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    mut update_receiver: Option<mpsc::Receiver<(StateGraph, Vec<crate::data_source::Transaction>)>>,
) -> Result<()> {
    loop {
        // Check for updates
        if let Some(rx) = &mut update_receiver
            && let Ok((new_graph, new_txs)) = rx.try_recv()
        {
            app.update_data(new_graph, new_txs);
        }

        terminal
            .draw(|f| ui::draw(f, &mut app))
            .map_err(|e| crate::Error::Tui(e.to_string()))?;

        if event::poll(Duration::from_millis(100)).map_err(|e| crate::Error::Tui(e.to_string()))?
            && let Event::Key(key) = event::read().map_err(|e| crate::Error::Tui(e.to_string()))?
        {
            match key.code {
                KeyCode::Char('q') => {
                    app.quit();
                }
                KeyCode::Char('h') | KeyCode::Char('?') => {
                    app.set_view_mode(ViewMode::Help);
                }
                KeyCode::Char('g') => {
                    app.set_view_mode(ViewMode::GraphOverview);
                }
                KeyCode::Char('d') => {
                    app.set_view_mode(ViewMode::StateDetail);
                }
                KeyCode::Char('t') => {
                    app.set_view_mode(ViewMode::TransactionList);
                }
                KeyCode::Char('i') => {
                    app.set_view_mode(ViewMode::DatumInspector);
                }
                KeyCode::Char('p') => {
                    app.set_view_mode(ViewMode::PatternAnalysis);
                }
                KeyCode::Char('x') => {
                    // Toggle hex view in datum inspector
                    app.toggle_hex_view();
                }
                KeyCode::Tab => {
                    // Cycle through views
                    let next_mode = match app.view_mode {
                        ViewMode::GraphOverview => ViewMode::StateDetail,
                        ViewMode::StateDetail => ViewMode::TransactionList,
                        ViewMode::TransactionList => ViewMode::DatumInspector,
                        ViewMode::DatumInspector => ViewMode::PatternAnalysis,
                        ViewMode::PatternAnalysis => ViewMode::Help,
                        ViewMode::Help => ViewMode::GraphOverview,
                    };
                    app.set_view_mode(next_mode);
                }
                KeyCode::Up => {
                    // Context-aware navigation
                    match app.view_mode {
                        ViewMode::GraphOverview | ViewMode::PatternAnalysis => {
                            app.select_previous()
                        }
                        ViewMode::TransactionList => app.select_previous_transaction(),
                        _ => {} // Do nothing for other views
                    }
                }
                KeyCode::Down => {
                    // Context-aware navigation
                    match app.view_mode {
                        ViewMode::GraphOverview | ViewMode::PatternAnalysis => app.select_next(),
                        ViewMode::TransactionList => app.select_next_transaction(),
                        _ => {} // Do nothing for other views
                    }
                }

                KeyCode::Enter => match app.view_mode {
                    ViewMode::TransactionList => app.set_view_mode(ViewMode::DatumInspector),
                    _ => app.set_view_mode(ViewMode::StateDetail),
                },
                KeyCode::Esc => {
                    app.pop_view_mode();
                }
                _ => {}
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
