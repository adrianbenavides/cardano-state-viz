//! TUI UI rendering

use super::app::{App, ViewMode};
use crate::state_machine::{StateClass, analyzer::ContractPattern};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

/// Draw the UI based on current app state
pub fn draw(f: &mut Frame, app: &mut App) {
    match app.view_mode {
        ViewMode::GraphOverview => draw_graph_overview(f, app),
        ViewMode::StateDetail => draw_state_detail(f, app),
        ViewMode::TransactionList => draw_transaction_list(f, app),
        ViewMode::DatumInspector => draw_datum_inspector(f, app),
        ViewMode::PatternAnalysis => draw_pattern_analysis(f, app),
        ViewMode::Help => draw_help(f),
    }
}

fn draw_graph_overview(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // State list
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new("Cardano State Machine Visualizer - Graph Overview")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Calculate items and stats in a separate block to release immutable borrow of app
    let (items, stats) = {
        let states_list = app.states_list();
        let items: Vec<ListItem> = states_list
            .iter()
            .enumerate()
            .map(|(idx, state_id)| {
                let state = app.state_graph.get_state(state_id).unwrap();
                let is_selected = idx == app.selected_state_index;

                let color = match state.metadata.classification {
                    StateClass::Initial => Color::LightBlue,
                    StateClass::Active => Color::Yellow,
                    StateClass::Completed => Color::Green,
                    StateClass::Failed => Color::Red,
                    StateClass::Locked => Color::Magenta,
                    StateClass::Unknown => Color::Gray,
                };

                let prefix = if is_selected { "► " } else { "  " };
                let text = format!(
                    "{}{} | Block: {} | Slot: {} | {} ADA",
                    prefix,
                    state.id,
                    state.block,
                    state.slot,
                    state.ada_value() as f64 / 1_000_000.0
                );

                let style = if is_selected {
                    Style::default()
                        .fg(color)
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::DarkGray)
                } else {
                    Style::default().fg(color)
                };

                ListItem::new(text).style(style)
            })
            .collect();

        (items, app.state_graph.stats())
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("States"))
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, chunks[1], &mut app.state_list_state);

    // Scrollbar
    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));
    let mut scrollbar_state = ScrollbarState::new(stats.total_states).position(app.selected_state_index);
    f.render_stateful_widget(
        scrollbar,
        chunks[1].inner(ratatui::layout::Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );

    // Footer with stats and instructions
    let footer_text = format!(
        "[{}/{}] States | Transitions: {} | Initial: {} | Terminal: {} | [↑/↓] Navigate | [Enter/d] Detail | [h/?] Help | [q] Quit",
        app.selected_state_index + 1, stats.total_states, stats.total_transitions, stats.initial_states, stats.terminal_states
    );
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

/// Draw detailed view of selected state
fn draw_state_detail(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Details
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new("State Detail View")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Details
    if let Some(state) = app.get_selected_state() {
        let detail_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(40), // State info
                Constraint::Percentage(30), // Incoming transitions
                Constraint::Percentage(30), // Outgoing transitions
            ])
            .split(chunks[1]);

        // State info
        let state_info = format_state_info(state);
        let state_widget = Paragraph::new(state_info)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("State Information"),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(state_widget, detail_chunks[0]);

        // Incoming transitions
        let incoming = app.state_graph.incoming_transitions(&state.id);
        let incoming_text = if incoming.is_empty() {
            "No incoming transitions (Initial state)".to_string()
        } else {
            incoming
                .iter()
                .map(|t| format!("← {} (tx: {})", t.from_state, &t.tx_hash[..8]))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let incoming_widget = Paragraph::new(incoming_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Incoming Transitions"),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(incoming_widget, detail_chunks[1]);

        // Outgoing transitions
        let outgoing = app.state_graph.outgoing_transitions(&state.id);
        let outgoing_text = if outgoing.is_empty() {
            "No outgoing transitions (Terminal state)".to_string()
        } else {
            outgoing
                .iter()
                .map(|t| format!("→ {} (tx: {})", t.to_state, &t.tx_hash[..8]))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let outgoing_widget = Paragraph::new(outgoing_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Outgoing Transitions"),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(outgoing_widget, detail_chunks[2]);
    } else {
        let no_selection =
            Paragraph::new("No state selected").block(Block::default().borders(Borders::ALL));
        f.render_widget(no_selection, chunks[1]);
    }

    // Footer
    let footer =
        Paragraph::new("[↑/↓] Navigate | [g/Esc] Back to Overview | [h/?] Help | [q] Quit")
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

/// Draw transaction list view
fn draw_transaction_list(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Transaction list
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new("Transaction List")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Transaction list
    let (items, tx_count) = {
        let transactions = app.transactions();
        let items: Vec<ListItem> = transactions
            .iter()
            .enumerate()
            .map(|(idx, tx)| {
                let is_selected = idx == app.selected_transaction_index;

                // Count inputs/outputs at script address
                let script_inputs = tx
                    .inputs
                    .iter()
                    .filter(|i| {
                        app.state_graph
                            .state_index
                            .contains_key(&i.utxo_ref.to_string())
                    })
                    .count();

                let script_outputs = tx
                    .outputs
                    .iter()
                    .filter(|o| o.address == app.state_graph.script_address)
                    .count();

                let prefix = if is_selected { "► " } else { "  " };
                let text = format!(
                    "{}{} | Block: {} | Slot: {} | In: {} Out: {}",
                    prefix,
                    &tx.hash[..16],
                    tx.block,
                    tx.slot,
                    script_inputs,
                    script_outputs
                );

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(text).style(style)
            })
            .collect();
        (items, transactions.len())
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Transactions"))
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, chunks[1], &mut app.transaction_list_state);

    // Scrollbar
    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));
    let mut scrollbar_state = ScrollbarState::new(tx_count).position(app.selected_transaction_index);
    f.render_stateful_widget(
        scrollbar,
        chunks[1].inner(ratatui::layout::Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );

    // Footer
    let footer_text = format!(
        "[{}/{}] Transactions | [↑/↓] Navigate | [Enter/i] Inspect Datum | [g] Graph | [d] Details | [h/?] Help | [q] Quit",
        app.selected_transaction_index + 1, tx_count
    );
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

/// Draw datum inspector view
fn draw_datum_inspector(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Datum content
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Header
    let view_type = if app.show_hex_view {
        "Hex View"
    } else {
        "Decoded View"
    };
    let header = Paragraph::new(format!("Datum Inspector - {}", view_type))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Datum content
    let content = if let Some(tx) = app.get_selected_transaction() {
        let mut datum_text = String::new();
        datum_text.push_str(&format!("Transaction: {}\n", tx.hash));
        datum_text.push_str(&format!("Block: {} | Slot: {}\n\n", tx.block, tx.slot));

        // Extract datums from outputs
        let datum_extractor = crate::parser::datum::DatumExtractor::new();
        match datum_extractor.extract_all_datums(tx) {
            Ok(datums) => {
                if datums.is_empty() {
                    datum_text.push_str("No datums found in this transaction.\n");
                } else {
                    for (output_idx, datum) in datums {
                        datum_text.push_str(&format!("Output #{}: ", output_idx));

                        if app.show_hex_view {
                            // Hex view
                            datum_text.push_str(&format!("Hash: {}\n", datum.hash));
                            datum_text.push_str("CBOR (hex): ");
                            for byte in &datum.raw_cbor {
                                datum_text.push_str(&format!("{:02x}", byte));
                            }
                            datum_text.push('\n');
                        } else {
                            // Decoded view
                            if let Some(ref parsed) = datum.parsed {
                                if !parsed.fields.is_empty() {
                                    datum_text.push_str("Schema Fields:\n");
                                    for (key, val) in &parsed.fields {
                                        datum_text.push_str(&format!("  {}: {}\n", key, val));
                                    }
                                    datum_text.push('\n');
                                }
                                datum_text.push_str(&format!(
                                    "Raw: {}\n",
                                    parsed.raw.to_human_readable()
                                ));
                            } else {
                                datum_text
                                    .push_str(&format!("Hash: {} (not parsed)\n", datum.hash));
                            }
                        }
                        datum_text.push('\n');
                    }
                }
            }
            Err(e) => {
                datum_text.push_str(&format!("Error extracting datums: {}\n", e));
            }
        }

        datum_text
    } else {
        "No transaction selected.\nNavigate to a transaction in the Transaction List view first."
            .to_string()
    };

    let content_widget = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title("Datum Data"))
        .wrap(Wrap { trim: false });
    f.render_widget(content_widget, chunks[1]);

    // Footer
    let footer_text =
        "[x] Toggle Hex/Decoded | [t] Transaction List | [g] Graph | [h/?] Help | [q] Quit";
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

/// Draw pattern analysis view
fn draw_pattern_analysis(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(5), // Metrics
            Constraint::Min(0),    // Visualization
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new("Pattern Analysis")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Metrics
    let report = &app.analysis_report;
    let metrics_text = format!(
        "Detected Pattern: {}\nBranching Factor: {:.2} | Max Depth: {} | Has Cycles: {}",
        report.pattern.display_name(),
        report.branching_factor,
        report.max_depth,
        report.has_cycles
    );
    let metrics = Paragraph::new(metrics_text)
        .block(Block::default().borders(Borders::ALL).title("Metrics"))
        .wrap(Wrap { trim: false });
    f.render_widget(metrics, chunks[1]);

    // Visualization
    let viz_title = match report.pattern {
        ContractPattern::Linear => "Timeline View (Linear)",
        ContractPattern::Tree => "Branching View (Tree)",
        ContractPattern::Cyclic => "Cycle View",
        ContractPattern::Unknown => "Graph View",
    };

    // For now, render a list of states with custom formatting based on pattern
    let (items, count) = {
        let states_list = app.states_list();
        let items: Vec<ListItem> = states_list
            .iter()
            .enumerate()
            .map(|(idx, state_id)| {
                let state = app.state_graph.get_state(state_id).unwrap();
                let is_selected = idx == app.selected_state_index;

                let content = match report.pattern {
                    ContractPattern::Linear => {
                        format!(
                            "{} | Block {} | {}",
                            state.id,
                            state.block,
                            state.display_short()
                        )
                    }
                    ContractPattern::Tree => {
                        // TODO: Calculate depth for indentation
                        format!("{} | {}", state.id, state.display_short())
                    }
                    _ => format!("{} | {}", state.id, state.display_short()),
                };

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                ListItem::new(content).style(style)
            })
            .collect();
        (items, states_list.len())
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(viz_title))
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, chunks[2], &mut app.state_list_state);

    // Scrollbar
    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));
    let mut scrollbar_state = ScrollbarState::new(count).position(app.selected_state_index);
    f.render_stateful_widget(
        scrollbar,
        chunks[2].inner(ratatui::layout::Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );

    // Footer
    let footer = Paragraph::new(format!("[{}/{}] | [Tab] Cycle Views | [q] Quit", app.selected_state_index + 1, count))
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[3]);
}

/// Draw help screen
fn draw_help(f: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Help content
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new("Help")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Help content
    let help_text = vec![
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ↑/↓          - Navigate through items (context-aware)"),
        Line::from("  Enter        - Open detail view (context-aware)"),
        Line::from("  Esc          - Return to graph overview"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Views",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  g            - Graph overview (state list)"),
        Line::from("  d            - State detail view"),
        Line::from("  t            - Transaction list"),
        Line::from("  i            - Datum inspector"),
        Line::from("  p            - Pattern analysis (via Tab cycling)"),
        Line::from("  h or ?       - This help screen"),
        Line::from("  Tab          - Cycle through views"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Datum Inspector",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  x            - Toggle hex/decoded view"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "General",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("  q            - Quit application"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "State Colors",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("■", Style::default().fg(Color::LightBlue)),
            Span::raw(" Initial    - No incoming transitions"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("■", Style::default().fg(Color::Yellow)),
            Span::raw(" Active     - Has both incoming and outgoing"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("■", Style::default().fg(Color::Green)),
            Span::raw(" Completed  - No outgoing transitions"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("■", Style::default().fg(Color::Red)),
            Span::raw(" Failed     - Error state"),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("■", Style::default().fg(Color::Magenta)),
            Span::raw(" Locked     - Temporarily locked"),
        ]),
    ];

    let help_widget = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Keyboard Shortcuts & Legend"),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(help_widget, chunks[1]);

    // Footer
    let footer = Paragraph::new("[g/Esc] Back to Overview | [q] Quit")
        .style(Style::default().fg(Color::White))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

/// Format state information for detail view
fn format_state_info(state: &crate::state_machine::State) -> String {
    let mut info = String::new();

    info.push_str(&format!("ID: {}\n", state.id));
    info.push_str(&format!(
        "Classification: {:?}\n",
        state.metadata.classification
    ));
    info.push_str(&format!("Block: {}\n", state.block));
    info.push_str(&format!("Slot: {}\n", state.slot));
    info.push_str(&format!("Transaction: {}\n", state.tx_hash));
    info.push_str(&format!(
        "ADA Value: {} ADA\n",
        state.ada_value() as f64 / 1_000_000.0
    ));

    if let Some(ref datum) = state.datum {
        info.push_str(&format!("\nDatum Hash: {}\n", datum.hash));
        info.push_str(&format!("Datum CBOR: {} bytes\n", datum.raw_cbor.len()));
        if let Some(ref parsed) = datum.parsed {
            if !parsed.fields.is_empty() {
                info.push_str("Schema Fields:\n");
                for (key, val) in &parsed.fields {
                    info.push_str(&format!("  {}: {}\n", key, val));
                }
            }
            info.push_str(&format!("Raw: {}\n", parsed.raw.to_human_readable()));
        }
    } else {
        info.push_str("\nNo datum\n");
    }

    // Output details
    info.push_str(&format!("\nOutput Address: {}\n", state.output.address));
    info.push_str("Assets:\n");
    for asset in &state.output.amount {
        info.push_str(&format!("  {} {}\n", asset.quantity, asset.unit));
    }

    info
}
