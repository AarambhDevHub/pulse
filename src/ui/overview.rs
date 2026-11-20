//! Overview tab: CPU history, network sparklines, memory and per-core bars.

use ratatui::prelude::*;
use ratatui::symbols::Marker;
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, Sparkline};

use super::widgets::{block, usage_bar};
use crate::app::App;
use crate::history::{HISTORY_LEN, RingBuf};
use crate::util::{fmt_bytes, fmt_rate};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).areas(area);
    let [cpu_area, net_area] =
        Layout::vertical([Constraint::Percentage(62), Constraint::Percentage(38)]).areas(left);
    let [mem_area, cores_area] =
        Layout::vertical([Constraint::Length(4), Constraint::Min(3)]).areas(right);

    render_cpu_chart(frame, cpu_area, app);
    render_network(frame, net_area, app);
    render_memory(frame, mem_area, app);
    render_cores(frame, cores_area, app);
}

fn render_cpu_chart(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let snap = &app.snapshot;

    // Newest sample sits at the right edge; older ones scroll off the left.
    let len = app.history.cpu_total.len();
    let points: Vec<(f64, f64)> = app
        .history
        .cpu_total
        .iter()
        .enumerate()
        .map(|(i, v)| ((HISTORY_LEN - len + i) as f64, *v))
        .collect();

    let dataset = Dataset::default()
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::new().fg(t.level(f64::from(snap.cpu_total))))
        .data(&points);

    let title = if snap.cpu_brand.is_empty() {
        format!(" CPU {:.0}% ", snap.cpu_total)
    } else {
        format!(" CPU {:.0}% · {} ", snap.cpu_total, snap.cpu_brand.trim())
    };

    let chart = Chart::new(vec![dataset])
        .block(block(title, t))
        .x_axis(Axis::default().bounds([0.0, (HISTORY_LEN - 1) as f64]))
        .y_axis(
            Axis::default()
                .bounds([0.0, 100.0])
                .labels(["0%", "50%", "100%"])
                .style(Style::new().fg(t.dim)),
        );
    frame.render_widget(chart, area);
}

fn render_network(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let [down, up] =
        Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

    let rx_now = app.history.net_rx.last().copied().unwrap_or(0);
    let tx_now = app.history.net_tx.last().copied().unwrap_or(0);

    for (rect, title, history, now, colour) in [
        (down, "↓ Download", &app.history.net_rx, rx_now, t.rx),
        (up, "↑ Upload", &app.history.net_tx, tx_now, t.tx),
    ] {
        let outer = block(format!(" {title} {} ", fmt_rate(now)), t);
        let inner = outer.inner(rect);
        frame.render_widget(outer, rect);

        // Left-pad with zeros so the newest sample sits at the right edge,
        // matching the line charts.
        let width = usize::from(inner.width);
        let recent = tail(history, width);
        let mut data = vec![0; width.saturating_sub(recent.len())];
        data.extend(recent);
        // Keep a floor on the scale so idle noise doesn't fill the bar.
        let max = data.iter().copied().max().unwrap_or(0).max(1024);
        frame.render_widget(
            Sparkline::default()
                .data(data)
                .max(max)
                .style(Style::new().fg(colour)),
            inner,
        );
    }
}

fn render_memory(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let mem = &app.snapshot.mem;
    let outer = block(" Memory ", t);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let [ram, swap] = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(inner);

    frame.render_widget(
        usage_bar(
            format!("RAM  {} / {}", fmt_bytes(mem.used), fmt_bytes(mem.total)),
            mem.used_ratio(),
            t,
        ),
        ram,
    );
    frame.render_widget(
        usage_bar(
            format!(
                "Swap {} / {}",
                fmt_bytes(mem.swap_used),
                fmt_bytes(mem.swap_total)
            ),
            mem.swap_ratio(),
            t,
        ),
        swap,
    );
}

/// One bar per core, flowing into extra columns when there are many cores.
fn render_cores(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let cores = &app.snapshot.cpu_cores;
    let outer = block(format!(" Cores ({}) ", cores.len()), t);
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = usize::from(inner.height);
    if rows == 0 || cores.is_empty() {
        return;
    }
    let columns = cores.len().div_ceil(rows).max(1);
    let per_column = cores.len().div_ceil(columns);
    let column_areas =
        Layout::horizontal(vec![Constraint::Ratio(1, columns as u32); columns]).split(inner);

    for (c, column) in column_areas.iter().enumerate() {
        for r in 0..per_column {
            let Some(&usage) = cores.get(c * per_column + r) else {
                break;
            };
            let cell = Rect {
                x: column.x,
                y: column.y + r as u16,
                width: column.width.saturating_sub(1),
                height: 1,
            };
            let label = format!("{:>2} {:>3.0}%", c * per_column + r, usage);
            frame.render_widget(usage_bar(label, f64::from(usage) / 100.0, t), cell);
        }
    }
}

/// The most recent `width` samples, oldest first.
fn tail(history: &RingBuf<u64>, width: usize) -> Vec<u64> {
    let skip = history.len().saturating_sub(width);
    history.iter().skip(skip).copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_newest_samples() {
        let mut r = RingBuf::new(10);
        for v in 1..=6 {
            r.push(v);
        }
        assert_eq!(tail(&r, 3), vec![4, 5, 6]);
        assert_eq!(tail(&r, 100), vec![1, 2, 3, 4, 5, 6]);
        assert!(tail(&r, 0).is_empty());
    }
}
