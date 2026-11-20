//! Events flowing into the main loop, and the thread that produces input events.

use std::sync::mpsc::Sender;
use std::thread;

use ratatui::crossterm::event::{self, Event as TermEvent, KeyEvent, KeyEventKind};

use crate::model::Snapshot;

/// Everything the main loop can receive.
pub enum Event {
    Key(KeyEvent),
    Resize,
    /// Fresh system data from the collector thread.
    Snapshot(Box<Snapshot>),
    /// A short message to show in the footer (for example the result of a kill).
    Notice(String),
}

/// Requests sent from the UI to the collector thread, which owns `sysinfo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Kill(u32),
}

/// Forward terminal input to the main loop until the channel closes.
pub fn spawn_input(tx: Sender<Event>) {
    thread::spawn(move || {
        loop {
            let event = match event::read() {
                // Windows also reports key releases; ignore them.
                Ok(TermEvent::Key(key)) if key.kind != KeyEventKind::Release => Event::Key(key),
                Ok(TermEvent::Resize(..)) => Event::Resize,
                Ok(_) => continue,
                Err(_) => break,
            };
            if tx.send(event).is_err() {
                break;
            }
        }
    });
}
