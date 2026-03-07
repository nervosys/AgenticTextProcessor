//! ATP TUI — Terminal User Interface for the Agentic Text Processor.
//!
//! Provides an interactive terminal interface optimized for human use with:
//! - Live search as you type
//! - Command completion and suggestions
//! - Operation explainers
//! - Result browsing with context
//! - Pipeline builder

mod app;
mod file_browser;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;

use app::App;

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new();
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                // Quit
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(());
                }
                KeyCode::Esc => {
                    if app.show_help {
                        app.show_help = false;
                    } else {
                        return Ok(());
                    }
                }

                // Toggle file browser
                KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.toggle_file_browser();
                }

                // Cycle focus
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.cycle_focus();
                }

                // Tab switching
                KeyCode::Tab => app.next_tab(),
                KeyCode::BackTab => app.prev_tab(),

                // Help
                KeyCode::F(1) => app.show_help = !app.show_help,

                // Input handling — depends on focus
                KeyCode::Char(c) => {
                    if app.focus == app::Focus::Input {
                        app.on_char(c);
                    }
                }
                KeyCode::Backspace => {
                    if app.focus == app::Focus::Input {
                        app.on_backspace();
                    }
                }
                KeyCode::Enter => {
                    match app.focus {
                        app::Focus::Input => app.on_enter(),
                        app::Focus::FileBrowser => app.on_file_browser_enter(),
                        app::Focus::Results => {} // Could open file at match location
                    }
                }

                // Navigation
                KeyCode::Up => match app.focus {
                    app::Focus::Input => {
                        if app.active_tab == app::Tab::Aql {
                            app.history_up();
                        }
                    }
                    app::Focus::Results => app.on_up(),
                    app::Focus::FileBrowser => app.file_browser.move_up(),
                },
                KeyCode::Down => match app.focus {
                    app::Focus::Input => {
                        if app.active_tab == app::Tab::Aql {
                            app.history_down();
                        }
                    }
                    app::Focus::Results => app.on_down(),
                    app::Focus::FileBrowser => app.file_browser.move_down(),
                },
                KeyCode::PageUp => app.on_page_up(),
                KeyCode::PageDown => app.on_page_down(),

                // Command completion
                KeyCode::Right => app.accept_suggestion(),

                _ => {}
            }
        }
    }
}
