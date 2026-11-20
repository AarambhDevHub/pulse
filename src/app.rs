//! Application state and the `update` half of the event loop.

use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;

use crate::event::{Command, Event};
use crate::history::History;
use crate::model::{ProcInfo, Snapshot};
use crate::theme::Theme;

/// Refresh intervals the `+` / `-` keys step through, in milliseconds.
pub const INTERVALS: [u64; 6] = [200, 500, 1000, 2000, 5000, 10_000];

/// How many rows PageUp / PageDown move.
const PAGE: isize = 10;

/// How long a footer notice stays visible.
const NOTICE_TTL: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Processes,
    Network,
    Disks,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Overview, Tab::Processes, Tab::Network, Tab::Disks];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Processes => "Processes",
            Tab::Network => "Network",
            Tab::Disks => "Disks",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    /// Typing into the process filter.
    Search,
    ConfirmKill {
        pid: u32,
        name: String,
    },
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Cpu,
    Mem,
    Pid,
    Name,
}

pub struct App {
    pub tab: Tab,
    pub mode: Mode,
    pub theme: Theme,
    pub should_quit: bool,

    pub snapshot: Snapshot,
    pub history: History,

    /// Process table state. `selected()` indexes into `view`.
    pub table_state: TableState,
    /// Indices into `snapshot.procs`, filtered and sorted for display.
    pub view: Vec<usize>,
    /// PID of the selected row, so the selection survives re-sorting.
    selected_pid: Option<u32>,
    pub sort: SortKey,
    pub sort_desc: bool,
    pub search: String,

    /// Current refresh interval, shared with the collector thread.
    pub interval_ms: Arc<AtomicU64>,
    commands: Sender<Command>,
    notice: Option<(String, Instant)>,
}

impl App {
    pub fn new(theme: Theme, interval_ms: Arc<AtomicU64>, commands: Sender<Command>) -> Self {
        Self {
            tab: Tab::Overview,
            mode: Mode::Normal,
            theme,
            should_quit: false,
            snapshot: Snapshot::default(),
            history: History::default(),
            table_state: TableState::default(),
            view: Vec::new(),
            selected_pid: None,
            sort: SortKey::Cpu,
            sort_desc: true,
            search: String::new(),
            interval_ms,
            commands,
            notice: None,
        }
    }

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.on_key(key),
            Event::Resize => {}
            Event::Snapshot(snapshot) => self.on_snapshot(*snapshot),
            Event::Notice(message) => self.notice = Some((message, Instant::now())),
        }
    }

    /// The footer notice, if it has not expired yet.
    pub fn notice(&self) -> Option<&str> {
        self.notice
            .as_ref()
            .filter(|(_, at)| at.elapsed() < NOTICE_TTL)
            .map(|(message, _)| message.as_str())
    }

    pub fn interval(&self) -> u64 {
        self.interval_ms.load(AtomicOrdering::Relaxed)
    }

    /// The process under the cursor, if any.
    pub fn selected_process(&self) -> Option<&ProcInfo> {
        let row = self.table_state.selected()?;
        let index = *self.view.get(row)?;
        self.snapshot.procs.get(index)
    }

    fn on_snapshot(&mut self, snapshot: Snapshot) {
        self.history.record(&snapshot);
        self.snapshot = snapshot;
        self.recompute_view();
    }

    // ---- keys ---------------------------------------------------------

    fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match self.mode.clone() {
            Mode::Normal => self.on_key_normal(key.code),
            Mode::Search => self.on_key_search(key.code),
            Mode::ConfirmKill { pid, .. } => self.on_key_confirm(key.code, pid),
            Mode::Help => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('?' | 'q')) {
                    self.mode = Mode::Normal;
                }
            }
        }
    }

    fn on_key_normal(&mut self, code: KeyCode) {
        let on_processes = self.tab == Tab::Processes;
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => {
                if !self.search.is_empty() {
                    self.search.clear();
                    self.recompute_view();
                }
            }
            KeyCode::Tab => self.tab = self.tab.next(),
            KeyCode::BackTab => self.tab = self.tab.prev(),
            KeyCode::Char(c @ '1'..='4') => {
                self.tab = Tab::ALL[usize::from(c as u8 - b'1')];
            }
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('+' | '=') => self.step_interval(true),
            KeyCode::Char('-' | '_') => self.step_interval(false),
            KeyCode::Char('/') => {
                self.tab = Tab::Processes;
                self.mode = Mode::Search;
            }
            KeyCode::Up | KeyCode::Char('k') if on_processes => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') if on_processes => self.move_selection(1),
            KeyCode::PageUp if on_processes => self.move_selection(-PAGE),
            KeyCode::PageDown if on_processes => self.move_selection(PAGE),
            KeyCode::Home | KeyCode::Char('g') if on_processes => self.select_row(0),
            KeyCode::End | KeyCode::Char('G') if on_processes => {
                self.select_row(self.view.len().saturating_sub(1));
            }
            KeyCode::Char('c') if on_processes => self.set_sort(SortKey::Cpu),
            KeyCode::Char('m') if on_processes => self.set_sort(SortKey::Mem),
            KeyCode::Char('p') if on_processes => self.set_sort(SortKey::Pid),
            KeyCode::Char('n') if on_processes => self.set_sort(SortKey::Name),
            KeyCode::Char('K') if on_processes => {
                if let Some(p) = self.selected_process() {
                    self.mode = Mode::ConfirmKill {
                        pid: p.pid,
                        name: p.name.clone(),
                    };
                }
            }
            _ => {}
        }
    }

    fn on_key_search(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.search.clear();
                self.mode = Mode::Normal;
                self.recompute_view();
            }
            KeyCode::Enter => self.mode = Mode::Normal,
            KeyCode::Backspace => {
                self.search.pop();
                self.recompute_view();
            }
            KeyCode::Char(c) => {
                self.search.push(c);
                self.recompute_view();
            }
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            _ => {}
        }
    }

    fn on_key_confirm(&mut self, code: KeyCode, pid: u32) {
        match code {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => {
                // The collector owns `sysinfo`, so it performs the kill.
                let _ = self.commands.send(Command::Kill(pid));
                self.mode = Mode::Normal;
            }
            KeyCode::Char('n' | 'N') | KeyCode::Esc => self.mode = Mode::Normal,
            _ => {}
        }
    }

    // ---- refresh rate -------------------------------------------------

    fn step_interval(&mut self, faster: bool) {
        let next = step_interval(self.interval(), faster);
        self.interval_ms.store(next, AtomicOrdering::Relaxed);
    }

    // ---- process table ------------------------------------------------

    fn set_sort(&mut self, key: SortKey) {
        if self.sort == key {
            self.sort_desc = !self.sort_desc;
        } else {
            self.sort = key;
            // Biggest first for numbers that matter, A-Z for names and PIDs.
            self.sort_desc = matches!(key, SortKey::Cpu | SortKey::Mem);
        }
        self.recompute_view();
    }

    fn move_selection(&mut self, delta: isize) {
        if self.view.is_empty() {
            return;
        }
        let current = self.table_state.selected().unwrap_or(0) as isize;
        let last = self.view.len() as isize - 1;
        self.select_row((current + delta).clamp(0, last) as usize);
    }

    fn select_row(&mut self, row: usize) {
        if self.view.is_empty() {
            self.table_state.select(None);
            self.selected_pid = None;
            return;
        }
        let row = row.min(self.view.len() - 1);
        self.table_state.select(Some(row));
        self.selected_pid = self.snapshot.procs.get(self.view[row]).map(|p| p.pid);
    }

    /// Rebuild `view` (filter + sort) and keep the same process selected.
    fn recompute_view(&mut self) {
        let needle = self.search.to_lowercase();
        let procs = &self.snapshot.procs;

        let mut view: Vec<usize> = procs
            .iter()
            .enumerate()
            .filter(|(_, p)| matches_filter(p, &needle))
            .map(|(i, _)| i)
            .collect();

        let (sort, desc) = (self.sort, self.sort_desc);
        view.sort_by(|&a, &b| {
            let ord = compare(&procs[a], &procs[b], sort);
            if desc { ord.reverse() } else { ord }
        });
        self.view = view;

        // Prefer the same PID; otherwise stay near the old row.
        let by_pid = self.selected_pid.and_then(|pid| {
            self.view
                .iter()
                .position(|&i| self.snapshot.procs[i].pid == pid)
        });
        let row = by_pid.unwrap_or_else(|| self.table_state.selected().unwrap_or(0));
        self.select_row(row);
    }
}

/// Move one step through [`INTERVALS`], clamping at both ends.
pub fn step_interval(current: u64, faster: bool) -> u64 {
    let idx = INTERVALS
        .iter()
        .position(|&v| v >= current)
        .unwrap_or(INTERVALS.len() - 1);
    let idx = if faster {
        idx.saturating_sub(1)
    } else if INTERVALS[idx] > current {
        // `current` sits between two presets: snap up to the next one.
        idx
    } else {
        (idx + 1).min(INTERVALS.len() - 1)
    };
    INTERVALS[idx]
}

fn matches_filter(p: &ProcInfo, needle_lower: &str) -> bool {
    needle_lower.is_empty()
        || p.name.to_lowercase().contains(needle_lower)
        || p.user.to_lowercase().contains(needle_lower)
        || p.pid.to_string().contains(needle_lower)
}

fn compare(a: &ProcInfo, b: &ProcInfo, key: SortKey) -> Ordering {
    let primary = match key {
        SortKey::Cpu => a.cpu.total_cmp(&b.cpu),
        SortKey::Mem => a.mem.cmp(&b.mem),
        SortKey::Pid => a.pid.cmp(&b.pid),
        SortKey::Name => a
            .name
            .bytes()
            .map(|c| c.to_ascii_lowercase())
            .cmp(b.name.bytes().map(|c| c.to_ascii_lowercase())),
    };
    // PID as tie-breaker keeps the order stable between refreshes.
    primary.then_with(|| a.pid.cmp(&b.pid))
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use ratatui::crossterm::event::KeyEventKind;

    use super::*;
    use crate::model::sample;

    fn fresh() -> (App, mpsc::Receiver<Command>) {
        let (tx, rx) = mpsc::channel();
        let mut app = App::new(Theme::dark(), Arc::new(AtomicU64::new(1000)), tx);
        app.handle_event(Event::Snapshot(Box::new(sample::snapshot())));
        (app, rx)
    }

    fn press(app: &mut App, code: KeyCode) {
        let mut key = KeyEvent::new(code, KeyModifiers::NONE);
        key.kind = KeyEventKind::Press;
        app.handle_event(Event::Key(key));
    }

    fn names(app: &App) -> Vec<&str> {
        app.view
            .iter()
            .map(|&i| app.snapshot.procs[i].name.as_str())
            .collect()
    }

    #[test]
    fn default_sort_is_cpu_descending() {
        let (app, _) = fresh();
        assert_eq!(
            names(&app),
            ["firefox", "rust-analyzer", "Code", "init", "zsh"]
        );
    }

    #[test]
    fn sort_keys_and_direction_toggle() {
        let (mut app, _) = fresh();
        press(&mut app, KeyCode::Tab); // -> Processes
        assert_eq!(app.tab, Tab::Processes);

        press(&mut app, KeyCode::Char('m'));
        assert_eq!(names(&app)[0], "firefox"); // most memory first

        press(&mut app, KeyCode::Char('n')); // name A-Z, case-insensitive
        assert_eq!(
            names(&app),
            ["Code", "firefox", "init", "rust-analyzer", "zsh"]
        );

        press(&mut app, KeyCode::Char('n')); // same key reverses
        assert_eq!(names(&app)[0], "zsh");

        press(&mut app, KeyCode::Char('p'));
        assert_eq!(names(&app)[0], "init"); // PID 1 first
    }

    #[test]
    fn sort_keys_do_nothing_outside_processes_tab() {
        let (mut app, _) = fresh();
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(app.sort, SortKey::Cpu);
    }

    #[test]
    fn selection_follows_pid_when_order_changes() {
        let (mut app, _) = fresh();
        app.tab = Tab::Processes;
        press(&mut app, KeyCode::Down); // rust-analyzer
        assert_eq!(app.selected_process().unwrap().name, "rust-analyzer");

        press(&mut app, KeyCode::Char('n')); // re-sort by name
        assert_eq!(app.selected_process().unwrap().name, "rust-analyzer");
    }

    #[test]
    fn selection_is_clamped() {
        let (mut app, _) = fresh();
        app.tab = Tab::Processes;
        press(&mut app, KeyCode::Up);
        assert_eq!(app.table_state.selected(), Some(0));
        press(&mut app, KeyCode::End);
        assert_eq!(app.table_state.selected(), Some(4));
        press(&mut app, KeyCode::PageDown);
        assert_eq!(app.table_state.selected(), Some(4));
        press(&mut app, KeyCode::Home);
        assert_eq!(app.table_state.selected(), Some(0));
    }

    #[test]
    fn search_filters_by_name_user_and_pid() {
        let (mut app, _) = fresh();
        press(&mut app, KeyCode::Char('/'));
        assert_eq!(app.mode, Mode::Search);
        assert_eq!(app.tab, Tab::Processes);

        for c in "RUST".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        assert_eq!(names(&app), ["rust-analyzer"]);

        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(names(&app), ["rust-analyzer"]); // filter is kept

        press(&mut app, KeyCode::Esc); // clears the filter
        assert_eq!(app.view.len(), 5);

        press(&mut app, KeyCode::Char('/'));
        for c in "alice".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        assert_eq!(app.view.len(), 4);

        press(&mut app, KeyCode::Esc); // cancels search and filter
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.view.len(), 5);

        press(&mut app, KeyCode::Char('/'));
        for c in "200".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        assert_eq!(names(&app), ["zsh"]);
    }

    #[test]
    fn q_is_typed_not_quit_while_searching() {
        let (mut app, _) = fresh();
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('q'));
        assert!(!app.should_quit);
        assert_eq!(app.search, "q");
    }

    #[test]
    fn empty_filter_result_clears_selection() {
        let (mut app, _) = fresh();
        press(&mut app, KeyCode::Char('/'));
        for c in "zzzz".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        assert!(app.view.is_empty());
        assert_eq!(app.table_state.selected(), None);
        assert!(app.selected_process().is_none());
    }

    #[test]
    fn kill_needs_confirmation() {
        let (mut app, rx) = fresh();
        app.tab = Tab::Processes;
        press(&mut app, KeyCode::Char('K'));
        assert_eq!(
            app.mode,
            Mode::ConfirmKill {
                pid: 42,
                name: "firefox".into()
            }
        );
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(app.mode, Mode::Normal);
        assert!(rx.try_recv().is_err());

        press(&mut app, KeyCode::Char('K'));
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(rx.try_recv(), Ok(Command::Kill(42)));
    }

    #[test]
    fn kill_key_only_works_on_processes_tab() {
        let (mut app, _) = fresh();
        press(&mut app, KeyCode::Char('K'));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn tabs_cycle_and_jump() {
        let (mut app, _) = fresh();
        press(&mut app, KeyCode::BackTab);
        assert_eq!(app.tab, Tab::Disks);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.tab, Tab::Overview);
        press(&mut app, KeyCode::Char('3'));
        assert_eq!(app.tab, Tab::Network);
        press(&mut app, KeyCode::Char('4'));
        assert_eq!(app.tab, Tab::Disks);
        press(&mut app, KeyCode::Char('9'));
        assert_eq!(app.tab, Tab::Disks);
    }

    #[test]
    fn quit_keys() {
        let (mut app, _) = fresh();
        press(&mut app, KeyCode::Char('q'));
        assert!(app.should_quit);

        let (mut app, _) = fresh();
        app.handle_event(Event::Key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
        assert!(app.should_quit);
    }

    #[test]
    fn help_popup_opens_and_closes() {
        let (mut app, _) = fresh();
        press(&mut app, KeyCode::Char('?'));
        assert_eq!(app.mode, Mode::Help);
        press(&mut app, KeyCode::Char('q')); // closes, does not quit
        assert_eq!(app.mode, Mode::Normal);
        assert!(!app.should_quit);
    }

    #[test]
    fn refresh_interval_steps() {
        assert_eq!(step_interval(1000, true), 500);
        assert_eq!(step_interval(1000, false), 2000);
        assert_eq!(step_interval(200, true), 200);
        assert_eq!(step_interval(10_000, false), 10_000);
        assert_eq!(step_interval(750, true), 500);
        assert_eq!(step_interval(750, false), 1000);

        let (mut app, _) = fresh();
        press(&mut app, KeyCode::Char('+'));
        assert_eq!(app.interval(), 500);
        press(&mut app, KeyCode::Char('-'));
        press(&mut app, KeyCode::Char('-'));
        assert_eq!(app.interval(), 2000);
    }

    #[test]
    fn notices_are_shown() {
        let (mut app, _) = fresh();
        assert_eq!(app.notice(), None);
        app.handle_event(Event::Notice("done".into()));
        assert_eq!(app.notice(), Some("done"));
    }

    #[test]
    fn history_grows_with_snapshots() {
        let (app, _) = fresh();
        assert_eq!(app.history.cpu_total.len(), 1);
    }
}
