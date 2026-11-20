//! Pulse: a terminal system monitor built with Ratatui.
//!
//! Threads:
//! - the collector owns `sysinfo` and sends [`event::Event::Snapshot`]s
//! - the input thread forwards key presses and resizes
//! - the main thread owns [`app::App`], applies events and redraws

mod app;
mod cli;
mod collector;
mod event;
mod history;
mod model;
mod splash;
mod theme;
mod ui;
mod util;

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, mpsc};

use clap::Parser;
use color_eyre::Result;
use ratatui::DefaultTerminal;

use crate::app::App;
use crate::cli::Cli;
use crate::collector::Collector;
use crate::theme::Theme;

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    // Fail with a normal error message *before* the terminal is taken over.
    let collector = Collector::new()?;

    // `init` switches to the alternate screen and installs a panic hook that
    // restores the terminal first, so a crash never leaves it garbled.
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &cli, collector);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, cli: &Cli, collector: Collector) -> Result<()> {
    let (event_tx, event_rx) = mpsc::channel();
    let (command_tx, command_rx) = mpsc::channel();
    let interval_ms = Arc::new(AtomicU64::new(cli.refresh));

    let theme = Theme::from(cli.theme);
    let input_tx = event_tx.clone();

    // Start collecting first, so real data is already waiting when the
    // splash ends.
    collector::spawn(collector, event_tx, command_rx, Arc::clone(&interval_ms));

    // The splash reads the keyboard itself, so it must finish before the
    // input thread starts competing for key events.
    if !cli.no_splash && splash::run(terminal, &theme)? == splash::Outcome::Quit {
        return Ok(());
    }
    event::spawn_input(input_tx);

    let mut app = App::new(theme, interval_ms, command_tx);

    while !app.should_quit {
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        // Block for one event, then drain whatever else is queued so a burst
        // of input costs one redraw, not one per event.
        let Ok(first) = event_rx.recv() else { break };
        app.handle_event(first);
        while let Ok(next) = event_rx.try_recv() {
            app.handle_event(next);
        }
    }
    Ok(())
}
