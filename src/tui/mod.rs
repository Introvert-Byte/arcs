pub mod app;
pub mod events;
pub mod ui;

use crate::error::Result;
use app::App;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use events::{next_event, AppEvent};
use ratatui::prelude::*;
use std::io::{self, Stdout};
use std::time::Duration;

pub fn run() -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new();

    let res = run_app(&mut terminal, &mut app);

    restore_terminal(&mut terminal)?;
    res
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if let Some(ev) = next_event(Duration::from_millis(100))? {
            match ev {
                AppEvent::Key(key) => {
                    app.handle_key(key);
                    if app.should_quit {
                        break;
                    }
                }
                AppEvent::Tick => {
                    // future: animate progress
                }
            }
        }
    }
    Ok(())
}