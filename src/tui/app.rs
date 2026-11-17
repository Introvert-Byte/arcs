use crate::discovery::{discover_all, DiscoveredItem};
use crate::error::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::ListState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Discover,
    BackupProgress,
    Inspector,
    Restore,
    Help,
}

pub struct App {
    pub screen: Screen,
    pub items: Vec<DiscoveredItem>,
    pub list_state: ListState,
    pub status: String,
    pub progress: f64,
    pub should_quit: bool,
    pub password: String,
    pub output_path: String,
    pub dry_run: bool,
}

impl App {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            screen: Screen::Home,
            items: Vec::new(),
            list_state,
            status: "Welcome to Arcs – press ? for help".into(),
            progress: 0.0,
            should_quit: false,
            password: String::new(),
            output_path: "backup.arcs".into(),
            dry_run: false,
        }
    }

    pub fn refresh_discovery(&mut self) -> Result<()> {
        self.status = "Scanning system…".into();
        self.items = discover_all()?;
        if !self.items.is_empty() {
            self.list_state.select(Some(0));
        }
        self.status = format!("Found {} items", self.items.len());
        Ok(())
    }

    pub fn selected_count(&self) -> usize {
        self.items.iter().filter(|i| i.selected).count()
    }

    pub fn toggle_selected(&mut self) {
        if let Some(i) = self.list_state.selected() {
            if let Some(item) = self.items.get_mut(i) {
                item.selected = !item.selected;
            }
        }
    }

    pub fn select_all(&mut self, value: bool) {
        for item in &mut self.items {
            item.selected = value;
        }
    }

    pub fn next(&mut self) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    len - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.screen {
            Screen::Home => match key.code {
                KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Char('d') | KeyCode::Enter => {
                    self.screen = Screen::Discover;
                    let _ = self.refresh_discovery();
                }
                KeyCode::Char('i') => self.screen = Screen::Inspector,
                KeyCode::Char('r') => self.screen = Screen::Restore,
                KeyCode::Char('?') => self.screen = Screen::Help,
                _ => {}
            },
            Screen::Discover => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.screen = Screen::Home,
                KeyCode::Down | KeyCode::Char('j') => self.next(),
                KeyCode::Up | KeyCode::Char('k') => self.previous(),
                KeyCode::Char(' ') => self.toggle_selected(),
                KeyCode::Char('a') => self.select_all(true),
                KeyCode::Char('n') => self.select_all(false),
                KeyCode::Enter => {
                    if self.selected_count() > 0 {
                        self.screen = Screen::BackupProgress;
                        self.status = format!(
                            "Ready to backup {} selected items → {}",
                            self.selected_count(),
                            self.output_path
                        );
                    } else {
                        self.status = "Select at least one item (Space)".into();
                    }
                }
                KeyCode::Char('?') => self.screen = Screen::Help,
                _ => {}
            },
            Screen::BackupProgress | Screen::Inspector | Screen::Restore | Screen::Help => {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => self.screen = Screen::Home,
                    _ => {}
                }
            }
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}