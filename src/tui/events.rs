use crossterm::event::{self, Event, KeyEventKind};
use std::time::Duration;

pub enum AppEvent {
    Key(crossterm::event::KeyEvent),
    Tick,
}

pub fn next_event(timeout: Duration) -> std::io::Result<Option<AppEvent>> {
    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => Ok(Some(AppEvent::Key(key))),
            _ => Ok(None),
        }
    } else {
        Ok(Some(AppEvent::Tick))
    }
}