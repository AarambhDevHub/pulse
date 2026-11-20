//! Disks tab: one usage gauge per mounted disk.

use ratatui::prelude::*;
use ratatui::widgets::Gauge;

use super::widgets::block;
use crate::app::App;
use crate::util::fmt_bytes;

/// Height of one disk entry (border + one gauge line + border).
const ENTRY_HEIGHT: u16 = 3;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let disks = &app.snapshot.disks;

    if disks.is_empty() {
        frame.render_widget(block(" Disks ", t), area);
        return;
    }

    // Show as many entries as fit; extra disks are cut off at the bottom.
    let fit = usize::from(area.height / ENTRY_HEIGHT).max(1);
    let shown = &disks[..disks.len().min(fit)];
    let areas = Layout::vertical(vec![Constraint::Length(ENTRY_HEIGHT); shown.len()]).split(area);

    for (disk, rect) in shown.iter().zip(areas.iter()) {
        let percent = disk.used_ratio() * 100.0;
        let name = if disk.name.is_empty() {
            "disk"
        } else {
            &disk.name
        };
        let title = format!(" {name}  {}  {}  {} ", disk.mount, disk.fs, disk.kind);
        let label = format!(
            "{} / {}  ({percent:.0}%)  free {}",
            fmt_bytes(disk.used()),
            fmt_bytes(disk.total),
            fmt_bytes(disk.available)
        );
        frame.render_widget(
            Gauge::default()
                .block(block(title, t))
                .gauge_style(Style::new().fg(t.level(percent)).bg(t.dim))
                .ratio(disk.used_ratio())
                .label(label),
            *rect,
        );
    }
}
