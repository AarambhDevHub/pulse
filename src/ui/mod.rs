//! Rendering. Draws the current `App` state; it never talks to `sysinfo`.
//! (It takes `&mut App` only because ratatui's `TableState` is mutated while
//! rendering a table.)

mod disks;
mod help;
mod network;
mod overview;
mod processes;
mod widgets;

use ratatui::prelude::*;
use ratatui::widgets::{Clear, Paragraph, Tabs};

use crate::app::{App, Mode, Tab};
use crate::util::fmt_duration;

/// Smallest terminal we lay out for.
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let message = format!(
            "Terminal too small ({}x{}).\nPulse needs at least {MIN_WIDTH}x{MIN_HEIGHT}.",
            area.width, area.height
        );
        frame.render_widget(Paragraph::new(message).alignment(Alignment::Center), area);
        return;
    }

    let [header, tabs, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    render_header(frame, header, app);
    render_tabs(frame, tabs, app);
    match app.tab {
        Tab::Overview => overview::render(frame, body, app),
        Tab::Processes => processes::render(frame, body, app),
        Tab::Network => network::render(frame, body, app),
        Tab::Disks => disks::render(frame, body, app),
    }
    render_footer(frame, footer, app);

    match app.mode.clone() {
        Mode::ConfirmKill { pid, name } => {
            processes::render_kill_popup(frame, area, app, pid, &name);
        }
        Mode::Help => {
            let popup = widgets::centered_rect(60, 21, area);
            frame.render_widget(Clear, popup);
            help::render(frame, popup, app);
        }
        Mode::Normal | Mode::Search => {}
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let sys = &app.snapshot.sys;
    let sep = Span::styled(" · ", Style::new().fg(t.dim));

    let mut spans = vec![Span::styled(" ◉ Pulse", Style::new().fg(t.accent).bold())];
    let mut push = |text: String| {
        spans.push(sep.clone());
        spans.push(Span::raw(text));
    };
    if !sys.hostname.is_empty() {
        push(sys.hostname.clone());
    }
    if !sys.os.is_empty() {
        push(sys.os.clone());
    }
    push(format!("up {}", fmt_duration(sys.uptime_secs)));
    push(format!(
        "load {:.2} {:.2} {:.2}",
        sys.load[0], sys.load[1], sys.load[2]
    ));
    push(format!("{} cores", app.snapshot.cpu_cores.len()));
    push(format!("refresh {} ms", app.interval()));

    frame.render_widget(Line::from(spans), area);
}

fn render_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let titles = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, tab)| format!(" {} {} ", i + 1, tab.title()));
    let tabs = Tabs::new(titles)
        .select(app.tab.index())
        .style(Style::new().fg(t.dim))
        .highlight_style(Style::new().fg(t.highlight_fg).bg(t.highlight_bg).bold())
        .divider("│");
    frame.render_widget(tabs, area);
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    if let Some(notice) = app.notice() {
        frame.render_widget(
            Line::styled(format!(" {notice}"), Style::new().fg(t.warn).bold()),
            area,
        );
        return;
    }

    let hints: &[(&str, &str)] = match (&app.mode, app.tab) {
        (Mode::Search, _) => &[("Enter", "keep filter"), ("Esc", "clear"), ("↑↓", "move")],
        (Mode::ConfirmKill { .. }, _) => &[("y", "kill"), ("n", "cancel")],
        (Mode::Help, _) => &[("Esc", "close help")],
        (Mode::Normal, Tab::Processes) => &[
            ("↑↓", "select"),
            ("c/m/p/n", "sort"),
            ("/", "filter"),
            ("K", "kill"),
            ("Tab", "next tab"),
            ("+/-", "refresh"),
            ("?", "help"),
            ("q", "quit"),
        ],
        (Mode::Normal, _) => &[
            ("Tab", "next tab"),
            ("1-4", "jump"),
            ("/", "find process"),
            ("+/-", "refresh"),
            ("?", "help"),
            ("q", "quit"),
        ],
    };

    let mut spans = vec![Span::raw(" ")];
    for (key, action) in hints {
        spans.push(Span::styled(*key, Style::new().fg(t.accent).bold()));
        spans.push(Span::styled(format!(" {action}  "), Style::new().fg(t.dim)));
    }
    frame.render_widget(Line::from(spans), area);
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU64;
    use std::sync::{Arc, mpsc};

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::event::Event;
    use crate::model::sample;
    use crate::theme::Theme;

    fn app() -> App {
        let (tx, _rx) = mpsc::channel();
        let mut app = App::new(Theme::dark(), Arc::new(AtomicU64::new(1000)), tx);
        app.handle_event(Event::Snapshot(Box::new(sample::snapshot())));
        app
    }

    /// Render `app` on a `width` x `height` terminal and return all text.
    fn draw(app: &mut App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| render(f, app)).unwrap();
        let buffer = terminal.backend().buffer();
        buffer
            .content()
            .chunks(usize::from(width))
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn overview_shows_header_and_panels() {
        let mut app = app();
        let text = draw(&mut app, 100, 30);
        for expected in [
            "Pulse",
            "testbox",
            "up 1d 1h 1m",
            "Overview",
            "Processes",
            "Network",
            "Disks",
            "CPU 42%",
            "Test CPU",
            "Memory",
            "Cores",
            "6.0 GiB / 16.0 GiB",
            "Swap",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
        }
    }

    #[test]
    fn processes_tab_lists_processes() {
        let mut app = app();
        app.tab = Tab::Processes;
        let text = draw(&mut app, 100, 30);
        for expected in [
            "PID",
            "firefox",
            "rust-analyzer",
            "alice",
            "35.5",
            "900.0 MiB",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
        }
    }

    #[test]
    fn processes_tab_shows_filter_line() {
        let mut app = app();
        // Type the filter through real key events so the view is refiltered.
        for c in "/fire".chars() {
            let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
            app.handle_event(Event::Key(key));
        }
        assert_eq!(app.tab, Tab::Processes);
        let text = draw(&mut app, 100, 30);
        assert!(text.contains("Filter: fire"), "{text}");
        assert!(!text.contains("rust-analyzer"));
    }

    #[test]
    fn network_tab_lists_interfaces() {
        let mut app = app();
        app.tab = Tab::Network;
        let text = draw(&mut app, 100, 30);
        for expected in ["eth0", "2.0 MiB/s", "512.0 KiB/s", "10.0 GiB"] {
            assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
        }
    }

    #[test]
    fn disks_tab_lists_disks() {
        let mut app = app();
        app.tab = Tab::Disks;
        let text = draw(&mut app, 100, 30);
        for expected in ["nvme0n1p2", "ext4", "SSD", "300.0 GiB / 500.0 GiB", "60%"] {
            assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
        }
    }

    #[test]
    fn kill_popup_names_the_process() {
        let mut app = app();
        app.tab = Tab::Processes;
        app.mode = Mode::ConfirmKill {
            pid: 42,
            name: "firefox".into(),
        };
        let text = draw(&mut app, 100, 30);
        assert!(text.contains("Kill firefox (PID 42)?"), "{text}");
    }

    #[test]
    fn help_popup_lists_keys() {
        let mut app = app();
        app.mode = Mode::Help;
        let text = draw(&mut app, 100, 30);
        assert!(text.contains("Keybindings"), "{text}");
        assert!(text.contains("Kill selected process"), "{text}");
    }

    #[test]
    fn small_terminal_shows_message_instead_of_panicking() {
        let mut app = app();
        let text = draw(&mut app, 40, 10);
        assert!(text.contains("Terminal too small"), "{text}");
    }

    #[test]
    fn empty_snapshot_renders_on_every_tab() {
        let (tx, _rx) = mpsc::channel();
        let mut app = App::new(Theme::mono(), Arc::new(AtomicU64::new(1000)), tx);
        for tab in Tab::ALL {
            app.tab = tab;
            draw(&mut app, 80, 24);
        }
    }

    #[test]
    fn many_cores_do_not_panic() {
        let mut app = app();
        let mut snap = sample::snapshot();
        snap.cpu_cores = (0..128).map(|i| (i % 100) as f32).collect();
        app.handle_event(Event::Snapshot(Box::new(snap)));
        draw(&mut app, 80, 24);
    }
}
