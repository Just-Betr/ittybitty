use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap},
};

use crate::app::{App, FilePickerState, FocusPanel, Mode, TorrentRow, View};

const COLOR_BG: Color = Color::Rgb(14, 16, 14);
const COLOR_PANEL: Color = Color::Rgb(20, 22, 20);
const COLOR_GREEN: Color = Color::Rgb(0, 245, 150);
const COLOR_BORDER: Color = Color::Rgb(0, 205, 110);
const COLOR_FOCUS_BG: Color = Color::Rgb(6, 8, 6);
const COLOR_CYAN: Color = Color::Rgb(0, 255, 255);
const COLOR_YELLOW: Color = Color::Rgb(255, 255, 0);
const COLOR_MUTED: Color = Color::Rgb(136, 136, 136);
const COLOR_BLACK: Color = Color::Rgb(0, 0, 0);

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let bg = Block::default().style(Style::default().bg(COLOR_BG));
    frame.render_widget(bg, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(10),
        ])
        .split(area);

    draw_top_bar(frame, layout[0]);
    draw_main(frame, layout[1], app);

    match app.mode() {
        Mode::EnterMagnet | Mode::EnterTorrentDir => draw_input_modal(frame, app),
        Mode::FilePicker => {
            if let Some(picker) = app.file_picker() {
                draw_file_picker(frame, picker);
            }
        }
        Mode::Normal => {}
    }

    if app.confirm_delete() {
        draw_confirm_delete(frame, app);
    }
    if app.confirm_quit() {
        draw_confirm_quit(frame, app);
    }

    if app.show_help() {
        draw_help_modal(frame, app.help_scroll());
    }

    if let Some(err) = app.last_error() {
        draw_error_modal(frame, err);
    }

}

fn draw_top_bar(frame: &mut Frame, area: Rect) {
    let block = Block::default().style(Style::default().bg(COLOR_GREEN).fg(COLOR_BLACK));
    frame.render_widget(block, area);

    let left = Line::from("IttyBitty - BitTorrent Client v0.1.0");
    let right = Line::from("[q: Quit] [?: Help]");

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    frame.render_widget(
        Paragraph::new(left)
            .style(Style::default().fg(COLOR_BLACK))
            .alignment(Alignment::Left),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(right)
            .style(Style::default().fg(COLOR_BLACK))
            .alignment(Alignment::Right),
        chunks[1],
    );
}

fn draw_main(frame: &mut Frame, area: Rect, app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(10)])
        .split(area);
    draw_sidebar(frame, columns[0], app);
    draw_right_panel(frame, columns[1], app);
}

fn draw_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(COLOR_BORDER))
        .style(Style::default().bg(COLOR_BG));
    frame.render_widget(&block, area);

    let inner = block.inner(area);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(8),
            Constraint::Min(1),
        ])
        .split(inner);

    draw_stats_panel(frame, sections[0], app, app.focus());
    draw_filters_panel(frame, sections[1], app);
    draw_keys_panel(frame, sections[2]);
}

fn draw_stats_panel(frame: &mut Frame, area: Rect, app: &App, _focus: FocusPanel) {
    let stats = app.session_stats();
    let down = stats
        .map(|s| format!("{}", s.download_speed))
        .unwrap_or_else(|| "-".to_string());
    let up = stats
        .map(|s| format!("{}", s.upload_speed))
        .unwrap_or_else(|| "-".to_string());

    let (active, seeding, total) = counts(app);
    let title_style = Style::default().fg(COLOR_BORDER);

    let lines = vec![
        Line::from(Span::styled("+- STATS -------------+", title_style)),
        Line::from(vec![
            Span::styled("| Global Down: ", Style::default().fg(COLOR_GREEN)),
            Span::styled(down, Style::default().fg(COLOR_CYAN)),
        ]),
        Line::from(vec![
            Span::styled("| Global Up:   ", Style::default().fg(COLOR_GREEN)),
            Span::styled(up, Style::default().fg(COLOR_YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("| Active:      ", Style::default().fg(COLOR_GREEN)),
            Span::styled(active.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("| Seeding:     ", Style::default().fg(COLOR_GREEN)),
            Span::styled(seeding.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("| Total:       ", Style::default().fg(COLOR_GREEN)),
            Span::styled(total.to_string(), Style::default().fg(Color::White)),
        ]),
        Line::from(Span::styled("+---------------------+", title_style)),
    ];

    let block = Block::default().style(Style::default().bg(COLOR_BG));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_filters_panel(frame: &mut Frame, area: Rect, app: &App) {
    let (downloading, seeding, paused, errors, total) = filter_counts(app);
    let selected = app.selected_filter();
    let focus = app.focus();
    let panel_bg = if focus == FocusPanel::Filters {
        COLOR_FOCUS_BG
    } else {
        COLOR_BG
    };
    let lines = vec![
        Line::from(Span::styled(
            "+- FILTERS -----------+",
            if focus == FocusPanel::Filters {
                Style::default().fg(COLOR_GREEN)
            } else {
                Style::default().fg(COLOR_BORDER)
            },
        )),
        filter_line(
            focus,
            selected,
            crate::app::FilterKind::All,
            format!("| [1] All Torrents ({total})"),
        ),
        filter_line(
            focus,
            selected,
            crate::app::FilterKind::Downloading,
            format!("| [2] Downloading ({downloading})"),
        ),
        filter_line(
            focus,
            selected,
            crate::app::FilterKind::Seeding,
            format!("| [3] Seeding ({seeding})"),
        ),
        filter_line(
            focus,
            selected,
            crate::app::FilterKind::Paused,
            format!("| [4] Paused ({paused})"),
        ),
        filter_line(
            focus,
            selected,
            crate::app::FilterKind::Stopped,
            "| [5] Stopped (0)".to_string(),
        ),
        filter_line(
            focus,
            selected,
            crate::app::FilterKind::Error,
            format!("| [6] Error ({errors})"),
        ),
        Line::from(Span::styled(
            "+---------------------+",
            if focus == FocusPanel::Filters {
                Style::default().fg(COLOR_GREEN)
            } else {
                Style::default().fg(COLOR_BORDER)
            },
        )),
    ];
    let block = Block::default().style(Style::default().bg(panel_bg));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_keys_panel(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "[TAB] Select Filters/Torrents",
            Style::default().fg(COLOR_CYAN),
        )),
        Line::from(Span::styled(
            "[↑/↓] Select",
            Style::default().fg(COLOR_MUTED),
        )),
        Line::from(Span::styled("[d] Delete", Style::default().fg(COLOR_MUTED))),
        Line::from(Span::styled(
            "[p] Pause/Resume",
            Style::default().fg(COLOR_MUTED),
        )),
        Line::from(Span::styled(
            "[a] Add torrent",
            Style::default().fg(COLOR_MUTED),
        )),
        Line::from(Span::styled(
            "[q] Quit",
            Style::default().fg(COLOR_MUTED),
        )),
        Line::from(Span::styled("[?] Help", Style::default().fg(COLOR_MUTED))),
        Line::from(Span::styled(
            "+---------------------+",
            Style::default().fg(COLOR_BORDER),
        )),
    ];
    let text = Text::from(lines);
    let block = Block::default().style(Style::default().bg(COLOR_BG));
    frame.render_widget(
        Paragraph::new(text).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_right_panel(frame: &mut Frame, area: Rect, app: &App) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(8),
            Constraint::Length(4),
        ])
        .split(area);

    draw_actions_bar(frame, sections[0]);
    match app.view() {
        View::Torrents => draw_table(frame, sections[1], app),
        View::Peers => draw_peers_view(frame, sections[1], app),
        View::Info => draw_info_view(frame, sections[1], app),
    }
    draw_selected_panel(frame, sections[2], app);
}

fn draw_actions_bar(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(COLOR_BORDER))
        .style(Style::default().bg(COLOR_BG));
    frame.render_widget(block, area);

    let left = Line::from(Span::styled(
        "View: [F]iles [V]Peers [I]nfo",
        Style::default().fg(COLOR_MUTED),
    ));
    let right = Line::from("");

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(100), Constraint::Percentage(0)])
        .split(area);

    frame.render_widget(
        Paragraph::new(left)
            .style(Style::default().fg(COLOR_GREEN))
            .alignment(Alignment::Left),
        chunks[0],
    );
    frame.render_widget(Paragraph::new(right).alignment(Alignment::Right), chunks[1]);
}

fn draw_table(frame: &mut Frame, area: Rect, app: &App) {
    let header_style = Style::default().fg(COLOR_BLACK).bg(COLOR_CYAN);
    let row_style = if app.focus() == FocusPanel::Torrents {
        Style::default().fg(COLOR_GREEN).bg(COLOR_FOCUS_BG)
    } else {
        Style::default().fg(COLOR_GREEN).bg(COLOR_BG)
    };

    let header = Row::new(vec![
        "NAME", " STATUS", " PROG%", " DOWN", " UP", " PEERS", " SIZE", " RATIO",
    ])
    .style(header_style)
    .height(1);

    let filtered: Vec<(usize, &TorrentRow)> = app
        .filtered_indices()
        .into_iter()
        .filter_map(|idx| app.torrents().get(idx).map(|t| (idx, t)))
        .collect();
    let col_widths = table_column_widths(area.width, 8);
    let rows: Vec<Row> = if filtered.is_empty() {
        vec![Row::new(vec![
            Cell::from(Text::from("No torrents in this filter")),
            Cell::from(Text::from(" ")),
            Cell::from(Text::from(" ")),
            Cell::from(Text::from(" ")),
            Cell::from(Text::from(" ")),
            Cell::from(Text::from(" ")),
            Cell::from(Text::from(" ")),
            Cell::from(Text::from(" ")),
        ])]
    } else {
        filtered
            .iter()
            .map(|(_, t)| torrent_row(t, &col_widths))
            .collect()
    };

    let table = Table::new(
        rows,
        vec![
            Constraint::Length(col_widths.get(0).copied().unwrap_or(0) as u16),
            Constraint::Length(col_widths.get(1).copied().unwrap_or(0) as u16),
            Constraint::Length(col_widths.get(2).copied().unwrap_or(0) as u16),
            Constraint::Length(col_widths.get(3).copied().unwrap_or(0) as u16),
            Constraint::Length(col_widths.get(4).copied().unwrap_or(0) as u16),
            Constraint::Length(col_widths.get(5).copied().unwrap_or(0) as u16),
            Constraint::Length(col_widths.get(6).copied().unwrap_or(0) as u16),
            Constraint::Length(col_widths.get(7).copied().unwrap_or(0) as u16),
        ],
    )
    .header(header)
    .block(Block::default().style(row_style))
    .highlight_symbol("")
    .row_highlight_style(match app.focus() {
        FocusPanel::Torrents => Style::default().bg(Color::Rgb(0, 30, 0)),
        FocusPanel::Filters => Style::default().bg(Color::Rgb(0, 60, 0)),
    })
    .column_spacing(0);

    let mut state = TableState::default();
    if !filtered.is_empty() {
        if let Some(pos) = filtered
            .iter()
            .position(|(idx, _)| *idx == app.selected_index())
        {
            state.select(Some(pos));
        }
    }
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_peers_view(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().style(Style::default().bg(COLOR_BG));
    let text = if let Some(t) = app.selected_torrent() {
        if let Some(stats) = t.stats.as_ref() {
            if let Some(live) = stats.live.as_ref() {
                let p = &live.snapshot.peer_stats;
                Text::from(vec![
                    Line::from(Span::styled("Peers", Style::default().fg(COLOR_GREEN))),
                    Line::from(""),
                    Line::from(format!("Live: {}", p.live)),
                    Line::from(format!("Seen: {}", p.seen)),
                    Line::from(format!("Queued: {}", p.queued)),
                    Line::from(format!("Connecting: {}", p.connecting)),
                    Line::from(format!("Dead: {}", p.dead)),
                ])
            } else {
                Text::from("No live peer data yet.")
            }
        } else {
            Text::from("No torrent selected.")
        }
    } else {
        Text::from("No torrent selected.")
    };
    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn draw_info_view(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().style(Style::default().bg(COLOR_BG));
    let text = if let Some(t) = app.selected_torrent() {
        let mut lines = vec![
            Line::from(Span::styled("Info", Style::default().fg(COLOR_GREEN))),
            Line::from(""),
            Line::from(format!("Name: {}", t.name)),
            Line::from(format!("Output: {}", t.output_folder)),
        ];
        if let Some(stats) = t.stats.as_ref() {
            lines.push(Line::from(format!(
                "Progress: {} / {}",
                format_bytes(stats.progress_bytes),
                format_bytes(stats.total_bytes)
            )));
            lines.push(Line::from(format!(
                "Uploaded: {}",
                format_bytes(stats.uploaded_bytes)
            )));
        }
        Text::from(lines)
    } else {
        Text::from("No torrent selected.")
    };
    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn draw_selected_panel(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(COLOR_BORDER))
        .style(Style::default().bg(COLOR_BG));
    frame.render_widget(&block, area);
    let inner = block.inner(area);

    let title = Line::from(Span::styled(
        "+- SELECTED TORRENT --------------------------------------------+",
        Style::default().fg(COLOR_GREEN),
    ));
    frame.render_widget(
        Paragraph::new(title),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );

    let (name, downloaded, eta) = selected_details(app);
    let line1 = Line::from(vec![
        Span::styled("Name: ", Style::default().fg(COLOR_MUTED)),
        Span::styled(name, Style::default().fg(Color::White)),
    ]);
    let line2 = Line::from(vec![
        Span::styled("Downloaded: ", Style::default().fg(COLOR_MUTED)),
        Span::styled(downloaded, Style::default().fg(COLOR_CYAN)),
    ]);
    let line3 = Line::from(vec![
        Span::styled("ETA: ", Style::default().fg(COLOR_MUTED)),
        Span::styled(eta, Style::default().fg(Color::White)),
    ]);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(Rect::new(
            inner.x,
            inner.y + 1,
            inner.width,
            inner.height - 1,
        ));

    frame.render_widget(Paragraph::new(line1), cols[0]);
    frame.render_widget(Paragraph::new(line2), cols[1]);
    frame.render_widget(Paragraph::new(line3), cols[2]);
}

fn draw_input_modal(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 20, frame.area());
    frame.render_widget(Clear, area);
    let title = match app.mode() {
        Mode::EnterMagnet => "Add torrent (magnet/URL/path)",
        Mode::EnterTorrentDir => "Torrent download directory (Enter to use)",
        _ => "Input",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(COLOR_GREEN))
        .style(Style::default().bg(COLOR_PANEL))
        .title(Span::styled(title, Style::default().fg(COLOR_GREEN)));
    let inner = block.inner(area);
    let (visible, cursor_x) = visible_input(app.input(), app.input_cursor(), inner.width);
    let paragraph = Paragraph::new(visible)
        .block(block)
        .style(Style::default().fg(Color::White));
    frame.render_widget(paragraph, area);
    if let Some(x) = cursor_x {
        let y = inner.y;
        frame.set_cursor_position((inner.x + x.saturating_sub(1), y));
    }
}

fn draw_file_picker(frame: &mut Frame, picker: &FilePickerState) {
    let area = centered_rect(90, 80, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(COLOR_GREEN))
        .style(Style::default().bg(COLOR_PANEL))
        .title(Span::styled(
            "Select files (space to toggle, a all, n none, Enter to start)",
            Style::default().fg(COLOR_GREEN),
        ));

    let rows: Vec<Row> = picker
        .files
        .iter()
        .enumerate()
        .map(|(idx, f)| {
            let checkbox = if f.included { "[x]" } else { "[ ]" };
            let style = if idx == picker.cursor {
                Style::default().bg(Color::Rgb(0, 120, 0)).fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };
            Row::new(vec![
                Span::raw(checkbox),
                Span::raw(" "),
                Span::raw(&f.name),
                Span::raw(" "),
                Span::raw(format_bytes(f.length)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Percentage(70),
            Constraint::Length(1),
            Constraint::Percentage(25),
        ],
    )
    .block(block)
    .column_spacing(0);

    frame.render_widget(table, area);
}

fn draw_error_modal(frame: &mut Frame, message: &str) {
    let area = centered_rect(70, 30, frame.area());
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .style(Style::default().bg(COLOR_BG))
        .title(Span::styled("Error", Style::default().fg(Color::Red)));

    let mut lines = vec![
        Line::from(Span::styled(
            "An error occurred",
            Style::default().fg(Color::Red),
        )),
        Line::from(""),
    ];
    if let Some((head, tail)) = message.split_once("Caused by:") {
        let head = head.trim();
        if !head.is_empty() {
            lines.push(Line::from(Span::styled(
                head,
                Style::default().fg(Color::White),
            )));
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            "Caused by:",
            Style::default().fg(Color::Red),
        )));
        let tail = tail.trim();
        if !tail.is_empty() {
            lines.push(Line::from(Span::styled(
                tail,
                Style::default().fg(Color::White),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            message,
            Style::default().fg(Color::White),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press x to dismiss",
        Style::default().fg(COLOR_MUTED),
    )));

    let text = Text::from(lines);

    let paragraph = Paragraph::new(text).block(block).alignment(Alignment::Left);
    frame.render_widget(paragraph, area);
}

fn draw_help_modal(frame: &mut Frame, scroll: u16) {
    let area = centered_rect(70, 40, frame.area());
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from(""),
        Line::from("Selection"),
        Line::from("  [TAB]  Select Filters/Torrents"),
        Line::from("  [↑/↓]  Select item"),
        Line::from(""),
        Line::from("Actions"),
        Line::from("  [d]  Delete (confirm dialog)"),
        Line::from("  [p]  Pause/Resume"),
        Line::from("  [a]  Add torrent"),
        Line::from(""),
        Line::from("Views"),
        Line::from("  [f]  Files"),
        Line::from("  [v]  Peers"),
        Line::from("  [i]  Info"),
        Line::from(""),
        Line::from("Exit"),
        Line::from("  [q]  Quit"),
        Line::from(""),
        Line::from("Press ? / x / Esc to close"),
    ];
    let text = Text::from(lines.clone());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(COLOR_CYAN))
        .style(Style::default().bg(COLOR_BG))
        .title(Span::styled("Help", Style::default().fg(COLOR_CYAN)));
    let inner = block.inner(area);
    let view_height = inner.height.saturating_sub(1) as usize;
    let max_scroll = lines.len().saturating_sub(view_height) as u16;
    let scroll = scroll.min(max_scroll);
    frame.render_widget(
        Paragraph::new(text).block(block).scroll((scroll, 0)),
        area,
    );
    if max_scroll > 0 {
        let indicator = format!("Scroll {}/{}", scroll, max_scroll);
        let indicator_area = Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(1),
            inner.width,
            1,
        );
        frame.render_widget(
            Paragraph::new(indicator)
                .style(Style::default().fg(COLOR_MUTED))
                .alignment(Alignment::Right),
            indicator_area,
        );
    }
}
fn filter_line(
    focus: FocusPanel,
    selected: crate::app::FilterKind,
    kind: crate::app::FilterKind,
    label: String,
) -> Line<'static> {
    let is_selected = selected == kind;
    let style = if is_selected {
        match focus {
            FocusPanel::Filters => Style::default().bg(COLOR_GREEN).fg(COLOR_BLACK),
            FocusPanel::Torrents => Style::default().bg(Color::Rgb(0, 70, 0)).fg(COLOR_GREEN),
        }
    } else {
        Style::default().fg(COLOR_MUTED)
    };
    let prefix = if is_selected { "> " } else { "  " };
    Line::from(Span::styled(format!("{prefix}{label}"), style))
}

fn draw_confirm_delete(frame: &mut Frame, app: &App) {
    let name = app
        .selected_torrent()
        .map(|t| t.name.as_str())
        .unwrap_or("-");
    let yes_style = if app.delete_choice() {
        Style::default().bg(Color::Yellow).fg(Color::Black)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let no_style = if app.delete_choice() {
        Style::default().fg(COLOR_MUTED)
    } else {
        Style::default().bg(Color::Yellow).fg(Color::Black)
    };
    let lines = vec![
        Line::from(Span::styled(
            "Delete files on disk too?",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(name, Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Y]es", yes_style),
            Span::raw("   "),
            Span::styled("[N]o", no_style),
        ]),
        Line::from(Span::styled(
            "[<-] [->] Select  [Enter] Confirm  [Esc] Cancel",
            Style::default().fg(COLOR_MUTED),
        )),
    ];
    let text = Text::from(lines.clone());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(COLOR_BG))
        .title(Span::styled("Confirm", Style::default().fg(Color::Yellow)));
    let area_height = ((lines.len() + 2) as u16)
        .min(frame.area().height.saturating_sub(2))
        .max(7);
    let area = centered_rect_fixed(70, area_height, frame.area());
    frame.render_widget(Clear, area);
    let inner = block.inner(area);
    let view_height = inner.height.saturating_sub(1) as usize;
    let scroll = lines.len().saturating_sub(view_height) as u16;
    frame.render_widget(
        Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
}

fn draw_confirm_quit(frame: &mut Frame, app: &App) {
    let yes_style = if app.quit_choice() {
        Style::default().bg(Color::Yellow).fg(Color::Black)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let no_style = if app.quit_choice() {
        Style::default().fg(COLOR_MUTED)
    } else {
        Style::default().bg(Color::Yellow).fg(Color::Black)
    };
    let lines = vec![
        Line::from(Span::styled(
            "Are you sure you want to quit?",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Y]es", yes_style),
            Span::raw("   "),
            Span::styled("[N]o", no_style),
        ]),
        Line::from(Span::styled(
            "[<-] [->] Select  [Enter] Confirm  [Esc] Cancel",
            Style::default().fg(COLOR_MUTED),
        )),
    ];
    let text = Text::from(lines.clone());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(COLOR_BG))
        .title(Span::styled("Confirm", Style::default().fg(Color::Yellow)));
    let area_height = ((lines.len() + 2) as u16)
        .min(frame.area().height.saturating_sub(2))
        .max(6);
    let area = centered_rect_fixed(70, area_height, frame.area());
    frame.render_widget(Clear, area);
    let inner = block.inner(area);
    let view_height = inner.height.saturating_sub(1) as usize;
    let scroll = lines.len().saturating_sub(view_height) as u16;
    frame.render_widget(
        Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
}

fn torrent_row(t: &TorrentRow, col_widths: &[usize]) -> Row<'static> {
    let (status, status_color) = format_status(t);
    let (prog, down, up, peers, size, ratio) = format_metrics(t);
    let spacing = 0usize;
    let gap_style = Style::default().fg(COLOR_GREEN);
    let status = format!("  {status}");
    let prog = format!("  {prog}");
    let down = format!("  {down}");
    let up = format!("  {up}");
    let peers = format!("  {peers}");
    let size = format!("  {size}");
    let ratio = format!("  {ratio}");
    let bar_len: usize = col_widths
        .iter()
        .sum::<usize>()
        .saturating_add(spacing * col_widths.len().saturating_sub(1))
        .max(1);
    let filled = progress_filled(t, bar_len);

    let name_width = col_widths.get(0).copied().unwrap_or(0);
    let status_width = col_widths.get(1).copied().unwrap_or(0);
    let prog_width = col_widths.get(2).copied().unwrap_or(0);
    let down_width = col_widths.get(3).copied().unwrap_or(0);
    let up_width = col_widths.get(4).copied().unwrap_or(0);
    let peers_width = col_widths.get(5).copied().unwrap_or(0);
    let size_width = col_widths.get(6).copied().unwrap_or(0);
    let ratio_width = col_widths.get(7).copied().unwrap_or(0);

    let name_text = fit_text(&t.name, name_width);
    let status = fit_text_padded(&status, status_width, 1);
    let prog = fit_text_padded(&prog, prog_width, 1);
    let down = fit_text_padded(&down, down_width, 1);
    let up = fit_text_padded(&up, up_width, 1);
    let peers = fit_text_padded(&peers, peers_width, 1);
    let size = fit_text_padded(&size, size_width, 1);
    let ratio = fit_text_padded(&ratio, ratio_width, 1);

    let name_cell = Text::from(vec![
        Line::from(Span::styled(
            name_text,
            Style::default().fg(COLOR_GREEN),
        )),
        bar_segment(
            filled,
            0,
            col_widths.get(0).copied().unwrap_or(0),
            spacing,
            gap_style,
        ),
    ]);
    let status_cell = Text::from(vec![
        Line::from(Span::styled(status, Style::default().fg(status_color))),
        bar_segment(
            filled,
            col_offset(col_widths, 1, spacing),
            col_widths.get(1).copied().unwrap_or(0),
            spacing,
            gap_style,
        ),
    ]);
    let prog_cell = Text::from(vec![
        Line::from(Span::styled(prog, Style::default().fg(COLOR_GREEN))),
        bar_segment(
            filled,
            col_offset(col_widths, 2, spacing),
            col_widths.get(2).copied().unwrap_or(0),
            spacing,
            gap_style,
        ),
    ]);
    let down_cell = Text::from(vec![
        Line::from(Span::styled(down, Style::default().fg(COLOR_CYAN))),
        bar_segment(
            filled,
            col_offset(col_widths, 3, spacing),
            col_widths.get(3).copied().unwrap_or(0),
            spacing,
            gap_style,
        ),
    ]);
    let up_cell = Text::from(vec![
        Line::from(Span::styled(up, Style::default().fg(COLOR_YELLOW))),
        bar_segment(
            filled,
            col_offset(col_widths, 4, spacing),
            col_widths.get(4).copied().unwrap_or(0),
            spacing,
            gap_style,
        ),
    ]);
    let peers_cell = Text::from(vec![
        Line::from(Span::styled(peers, Style::default().fg(COLOR_GREEN))),
        bar_segment(
            filled,
            col_offset(col_widths, 5, spacing),
            col_widths.get(5).copied().unwrap_or(0),
            spacing,
            gap_style,
        ),
    ]);
    let size_cell = Text::from(vec![
        Line::from(Span::styled(size, Style::default().fg(COLOR_GREEN))),
        bar_segment(
            filled,
            col_offset(col_widths, 6, spacing),
            col_widths.get(6).copied().unwrap_or(0),
            spacing,
            gap_style,
        ),
    ]);
    let ratio_cell = Text::from(vec![
        Line::from(Span::styled(ratio, Style::default().fg(COLOR_GREEN))),
        bar_segment(
            filled,
            col_offset(col_widths, 7, spacing),
            col_widths.get(7).copied().unwrap_or(0),
            spacing,
            gap_style,
        ),
    ]);

    Row::new(vec![
        Cell::from(name_cell),
        Cell::from(status_cell),
        Cell::from(prog_cell),
        Cell::from(down_cell),
        Cell::from(up_cell),
        Cell::from(peers_cell),
        Cell::from(size_cell),
        Cell::from(ratio_cell),
    ])
    .height(2)
}

fn table_column_widths(area_width: u16, _columns: usize) -> Vec<usize> {
    let spacing = 0;
    let fixed = 14 + 12 + 17 + 17 + 13 + 14 + 12;
    let available = area_width.saturating_sub(spacing as u16);
    let remaining = available.saturating_sub(fixed as u16);
    let first = remaining as usize;
    vec![first, 14, 12, 17, 17, 13, 14, 12]
}

fn progress_filled(t: &TorrentRow, width: usize) -> usize {
    let ratio = t
        .stats
        .as_ref()
        .and_then(|s| {
            if s.total_bytes == 0 {
                None
            } else {
                Some(s.progress_bytes as f64 / s.total_bytes as f64)
            }
        })
        .unwrap_or(0.0);
    let filled = (ratio * width as f64).round() as usize;
    filled.min(width)
}

fn bar_segment(
    filled: usize,
    start: usize,
    len: usize,
    gap: usize,
    gap_style: Style,
) -> Line<'static> {
    if len == 0 {
        return Line::from("");
    }
    let seg_filled = if filled <= start {
        0
    } else {
        (filled - start).min(len)
    };
    let seg_empty = len.saturating_sub(seg_filled);
    let mut spans = Vec::new();
    if seg_filled > 0 {
        spans.push(Span::styled(
            "\u{2588}".repeat(seg_filled),
            Style::default().fg(COLOR_GREEN),
        ));
    }
    if seg_empty > 0 {
        spans.push(Span::styled(
            "\u{2588}".repeat(seg_empty),
            Style::default().fg(COLOR_PANEL),
        ));
    }
    if gap > 0 {
        spans.push(Span::styled(" ".repeat(gap), gap_style));
    }
    Line::from(spans)
}

fn col_offset(widths: &[usize], idx: usize, spacing: usize) -> usize {
    widths.iter().take(idx).sum::<usize>() + spacing * idx
}

fn format_status(t: &TorrentRow) -> (String, Color) {
    let Some(stats) = t.stats.as_ref() else {
        return ("-".to_string(), COLOR_MUTED);
    };
    use librqbit::TorrentStatsState as S;
    match stats.state {
        S::Live => {
            if stats.finished {
                ("Seed".to_string(), COLOR_GREEN)
            } else {
                ("Down".to_string(), COLOR_CYAN)
            }
        }
        S::Initializing => ("Init".to_string(), COLOR_CYAN),
        S::Paused => ("Pause".to_string(), COLOR_YELLOW),
        S::Error => ("Error".to_string(), Color::Red),
    }
}

fn format_metrics(t: &TorrentRow) -> (String, String, String, String, String, String) {
    let Some(stats) = t.stats.as_ref() else {
        return (
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
        );
    };

    let prog = if stats.total_bytes == 0 {
        "-".to_string()
    } else {
        format!(
            "{:.0}%",
            (stats.progress_bytes as f64 / stats.total_bytes as f64) * 100.0
        )
    };

    let (down, up, peers) = if let Some(live) = stats.live.as_ref() {
        let p = &live.snapshot.peer_stats;
        let peer_text = if p.seen == 0 {
            "0/0".to_string()
        } else {
            format!("{}/{}", p.live, p.seen)
        };
        (
            format!("{}", live.download_speed),
            format!("{}", live.upload_speed),
            peer_text,
        )
    } else {
        ("0".to_string(), "0".to_string(), "0/0".to_string())
    };

    let size = format_bytes(stats.total_bytes);
    let ratio = if stats.progress_bytes == 0 {
        "-".to_string()
    } else {
        format!(
            "{:.2}",
            stats.uploaded_bytes as f64 / stats.progress_bytes as f64
        )
    };

    (prog, down, up, peers, size, ratio)
}

fn selected_details(app: &App) -> (String, String, String) {
    let Some(t) = app.selected_torrent() else {
        return ("-".to_string(), "-".to_string(), "-".to_string());
    };
    let name = t.name.clone();
    let (downloaded, eta) = if let Some(stats) = t.stats.as_ref() {
        let downloaded = format!(
            "{} / {}",
            format_bytes(stats.progress_bytes),
            format_bytes(stats.total_bytes)
        );
        let eta = stats
            .live
            .as_ref()
            .and_then(|l| l.time_remaining.as_ref())
            .map(|v| format!("{v}"))
            .unwrap_or_else(|| "-".to_string());
        (downloaded, eta)
    } else {
        ("-".to_string(), "-".to_string())
    };
    (name, downloaded, eta)
}

fn counts(app: &App) -> (usize, usize, usize) {
    let total = app.torrents().len();
    let mut active = 0;
    let mut seeding = 0;
    for t in app.torrents() {
        if let Some(stats) = t.stats.as_ref() {
            use librqbit::TorrentStatsState as S;
            let is_seeding = stats.finished
                || (stats.total_bytes > 0
                    && stats.progress_bytes >= stats.total_bytes
                    && matches!(stats.state, S::Live));
            match stats.state {
                S::Live | S::Initializing => {
                    active += 1;
                    if is_seeding {
                        seeding += 1;
                    }
                }
                _ => {}
            }
        }
    }
    (active, seeding, total)
}

fn filter_counts(app: &App) -> (usize, usize, usize, usize, usize) {
    let total = app.torrents().len();
    let mut downloading = 0;
    let mut seeding = 0;
    let mut paused = 0;
    let mut errors = 0;
    for t in app.torrents() {
        if let Some(stats) = t.stats.as_ref() {
            use librqbit::TorrentStatsState as S;
            let is_seeding = stats.finished
                || (stats.total_bytes > 0
                    && stats.progress_bytes >= stats.total_bytes
                    && matches!(stats.state, S::Live));
            match stats.state {
                S::Live | S::Initializing => {
                    if is_seeding {
                        seeding += 1;
                    } else {
                        downloading += 1;
                    }
                }
                S::Paused => paused += 1,
                S::Error => errors += 1,
            }
        }
    }
    (downloading, seeding, paused, errors, total)
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1}GB", b / GB)
    } else if b >= MB {
        format!("{:.1}MB", b / MB)
    } else if b >= KB {
        format!("{:.0}KB", b / KB)
    } else {
        format!("{}B", bytes)
    }
}

fn fit_text(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let count = s.chars().count();
    if count <= width {
        return s.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    let mut out = s.chars().take(width - 3).collect::<String>();
    out.push_str("...");
    out
}

fn fit_text_padded(s: &str, width: usize, pad: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let pad = pad.min(width);
    let inner = width.saturating_sub(pad);
    let trimmed = fit_text(s, inner);
    format!("{}{}", " ".repeat(pad), trimmed)
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

fn centered_rect_fixed(percent_x: u16, height: u16, r: Rect) -> Rect {
    let height = height.min(r.height.saturating_sub(2)).max(5);
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(height),
            Constraint::Min(0),
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

fn visible_input(input: &str, cursor: usize, area_width: u16) -> (String, Option<u16>) {
    let content_width = area_width as usize;
    if content_width == 0 {
        return (String::new(), None);
    }
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut start = 0;
    if cursor > content_width {
        start = cursor - content_width;
    }
    if start > len {
        start = len;
    }
    let end = (start + content_width).min(len);
    let visible: String = chars[start..end].iter().collect();
    let cursor_offset = cursor.saturating_sub(start);
    let cursor_x = (cursor_offset as u16).min(content_width as u16);
    (visible, Some(cursor_x + 1))
}

