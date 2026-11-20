//! Help popup listing every key binding.

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use super::widgets::block;
use crate::app::App;

const KEYS: &[(&str, &str)] = &[
    ("Tab / Shift+Tab", "Next / previous tab"),
    ("1 2 3 4", "Jump to a tab"),
    ("↑ ↓  j k", "Move selection (Processes)"),
    ("PgUp PgDn", "Move 10 rows"),
    ("Home End  g G", "First / last process"),
    ("c  m  p  n", "Sort by CPU / memory / PID / name"),
    ("", "Press the same key again to reverse"),
    ("/", "Filter by name, user or PID"),
    ("Esc", "Clear filter"),
    ("K", "Kill selected process (asks first)"),
    ("+  -", "Faster / slower refresh"),
    ("?", "Toggle this help"),
    ("q  Ctrl+C", "Quit"),
];

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let mut lines = vec![Line::from("")];
    for (key, action) in KEYS {
        lines.push(Line::from(vec![
            Span::styled(format!(" {key:<17}"), Style::new().fg(t.accent).bold()),
            Span::raw(*action),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        " Process CPU % is per total capacity (max 100).",
        Style::new().fg(t.dim),
    ));
    frame.render_widget(Paragraph::new(lines).block(block(" Keybindings ", t)), area);
}
