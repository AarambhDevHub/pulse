//! Processes tab: sortable, filterable table with a kill confirmation popup.

use ratatui::prelude::*;
use ratatui::widgets::{
    Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, Wrap,
};

use super::widgets::{block, centered_rect};
use crate::app::{App, Mode, SortKey};
use crate::util::fmt_bytes;

pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let t = app.theme;
    let show_filter = app.mode == Mode::Search || !app.search.is_empty();
    let [filter_area, table_area] = Layout::vertical([
        Constraint::Length(u16::from(show_filter)),
        Constraint::Min(0),
    ])
    .areas(area);

    if show_filter {
        let cursor = if app.mode == Mode::Search { "▏" } else { "" };
        frame.render_widget(
            Line::from(vec![
                Span::styled(" Filter: ", Style::new().fg(t.accent).bold()),
                Span::raw(format!("{}{cursor}", app.search)),
            ]),
            filter_area,
        );
    }

    let arrow = if app.sort_desc { " ▼" } else { " ▲" };
    let heading = |label: &str, key: Option<SortKey>| {
        let text = if key == Some(app.sort) {
            format!("{label}{arrow}")
        } else {
            label.to_owned()
        };
        Cell::from(text)
    };
    let header = Row::new([
        heading("PID", Some(SortKey::Pid)),
        heading("Name", Some(SortKey::Name)),
        heading("User", None),
        heading("CPU %", Some(SortKey::Cpu)),
        heading("Memory", Some(SortKey::Mem)),
        heading("Status", None),
    ])
    .style(Style::new().fg(t.accent).bold());

    let rows: Vec<Row> = app
        .view
        .iter()
        .map(|&i| {
            let p = &app.snapshot.procs[i];
            Row::new([
                Cell::from(p.pid.to_string()),
                Cell::from(p.name.clone()),
                Cell::from(p.user.clone()),
                Cell::from(format!("{:.1}", p.cpu))
                    .style(Style::new().fg(t.level(f64::from(p.cpu)))),
                Cell::from(fmt_bytes(p.mem)),
                Cell::from(p.status.clone()),
            ])
        })
        .collect();

    let title = format!(
        " Processes ({}/{}) ",
        app.view.len(),
        app.snapshot.procs.len()
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(16),
            Constraint::Length(14),
            Constraint::Length(9),
            Constraint::Length(11),
            Constraint::Length(9),
        ],
    )
    .header(header)
    .block(block(title, &t))
    .row_highlight_style(Style::new().fg(t.highlight_fg).bg(t.highlight_bg).bold())
    .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, table_area, &mut app.table_state);

    let mut scrollbar =
        ScrollbarState::new(app.view.len()).position(app.table_state.selected().unwrap_or(0));
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight).style(Style::new().fg(t.dim)),
        table_area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar,
    );
}

pub fn render_kill_popup(frame: &mut Frame, area: Rect, app: &App, pid: u32, name: &str) {
    let t = &app.theme;
    let popup = centered_rect(52, 6, area);
    frame.render_widget(Clear, popup);

    let text = vec![
        Line::from(format!("Kill {name} (PID {pid})?")).bold(),
        Line::from(""),
        Line::from(vec![
            Span::styled("y", Style::new().fg(t.bad).bold()),
            Span::raw(" kill   "),
            Span::styled("n", Style::new().fg(t.accent).bold()),
            Span::raw(" cancel"),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(block(" Confirm ", t).border_style(Style::new().fg(t.bad))),
        popup,
    );
}
