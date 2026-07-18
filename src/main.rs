mod crypto;
mod protocol;
mod network;
mod storage;
mod tui;
mod community;

use std::path::PathBuf;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use tracing::info;

use crate::crypto::identity::Identity;
use crate::storage::Storage;
use crate::tui::app::App;

fn data_dir() -> PathBuf {
    dirs_or_default()
}

fn dirs_or_default() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".root-chat")
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("root_chat=info")
        .init();

    let data_path = data_dir();
    std::fs::create_dir_all(&data_path)?;

    let storage = Storage::open(&data_path.join("db"))?;

    let identity = match storage.load_identity()? {
        Some(id) => {
            info!("Loaded existing identity: {}", id.address);
            id
        }
        None => {
            let id = Identity::generate();
            storage.save_identity(&id)?;
            info!("Generated new identity: {}", id.address);
            id
        }
    };

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(identity.address.clone());

    loop {
        terminal.draw(|frame| tui::ui::draw(frame, &app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char(c) => app.handle_key(c),
                        KeyCode::Enter => app.handle_key('\n'),
                        KeyCode::Esc => app.handle_key('\x1b'),
                        KeyCode::Backspace => {
                            app.input_buffer.pop();
                        }
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    println!("root-chat terminated. Identity: {}", identity.address);
    Ok(())
}
