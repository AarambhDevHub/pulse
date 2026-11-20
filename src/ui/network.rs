//! Network tab: aggregate throughput chart plus a per-interface table.

use ratatui::prelude::*;
use ratatui::symbols::Marker;
use ratatui::widgets::{Axis, Cell, Chart, Dataset, GraphType, Row, Table};

use super::widgets::block;
use crate::app::App;
use crate::history::{HISTORY_LEN, RingBuf};
use crate::util::{fmt_bytes, fmt_rate};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let [chart_area, table_area] =
        Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(area);
    render_chart(frame, chart_area, app);
    render_table(frame, table_area, app);
}

fn points(history: &RingBuf<u64>) -> Vec<(f64, f64)> {
    let len = history.len();
    history
        .iter()
        .enumerate()
        .map(|(i, v)| ((HISTORY_LEN - len + i) as f64, *v as f64))
        .collect()
}

fn render_chart(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let rx = points(&app.history.net_rx);
    let tx = points(&app.history.net_tx);

    let peak = app
        .history
        .net_rx
        .iter()
        .chain(app.history.net_tx.iter())
        .copied()
        .max()
        .unwrap_or(0)
        .max(1024);
    // Head-room so the line never touches the top border.
    let y_max = peak as f64 * 1.15;

    let datasets = vec![
        Dataset::default()
            .name("↓ rx")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::new().fg(t.rx))
            .data(&rx),
        Dataset::default()
            .name("↑ tx")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::new().fg(t.tx))
            .data(&tx),
    ];

    let rx_now = app.history.net_rx.last().copied().unwrap_or(0);
    let tx_now = app.history.net_tx.last().copied().unwrap_or(0);
    let title = format!(
        " Throughput  ↓ {}  ↑ {}  (loopback excluded) ",
        fmt_rate(rx_now),
        fmt_rate(tx_now)
    );

    let chart = Chart::new(datasets)
        .block(block(title, t))
        .x_axis(Axis::default().bounds([0.0, (HISTORY_LEN - 1) as f64]))
        .y_axis(
            Axis::default()
                .bounds([0.0, y_max])
                .labels([
                    "0".to_owned(),
                    fmt_rate((y_max / 2.0) as u64),
                    fmt_rate(y_max as u64),
                ])
                .style(Style::new().fg(t.dim)),
        );
    frame.render_widget(chart, area);
}

fn render_table(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let header = Row::new(["Interface", "↓ Rate", "↑ Rate", "↓ Total", "↑ Total"])
        .style(Style::new().fg(t.accent).bold());

    let rows: Vec<Row> = app
        .snapshot
        .nets
        .iter()
        .map(|n| {
            Row::new([
                Cell::from(n.name.clone()),
                Cell::from(fmt_rate(n.rx_rate)).style(Style::new().fg(t.rx)),
                Cell::from(fmt_rate(n.tx_rate)).style(Style::new().fg(t.tx)),
                Cell::from(fmt_bytes(n.rx_total)),
                Cell::from(fmt_bytes(n.tx_total)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(14),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(14),
        ],
    )
    .header(header)
    .block(block(
        format!(" Interfaces ({}) ", app.snapshot.nets.len()),
        t,
    ));
    frame.render_widget(table, area);
}
