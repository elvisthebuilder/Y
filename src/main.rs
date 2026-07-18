#![allow(dead_code)]

mod crypto;
mod protocol;
mod network;
mod storage;
mod tui;
mod community;

use std::path::PathBuf;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use tracing::info;

use crate::crypto::alias;
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

    let user_alias = match storage.load_alias()? {
        Some(a) => a,
        None => {
            let a = alias::generate_alias();
            storage.save_alias(&a)?;
            info!("Generated alias: {}", a);
            a
        }
    };
    let handle = alias::display_handle(&user_alias, &identity.address);

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(identity.address.clone(), handle.clone(), user_alias.clone());

    if let Ok(messages) = storage.get_timeline(100) {
        app.timeline = messages;
    }
    if let Ok(bookmarks) = storage.get_bookmarked_posts() {
        app.bookmarks = bookmarks;
    }

    loop {
        terminal.draw(|frame| tui::ui::draw(frame, &app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                    match key.code {
                        KeyCode::Char(c) => app.handle_key(c),
                        KeyCode::Enter if shift => app.insert_char('\n'),
                        KeyCode::Enter => app.handle_key('\n'),
                        KeyCode::Esc => app.handle_key('\x1b'),
                        KeyCode::Backspace => app.delete_char_before_cursor(),
                        KeyCode::Delete => app.delete_char_at_cursor(),
                        KeyCode::Left => app.move_cursor_left(),
                        KeyCode::Right => app.move_cursor_right(),
                        KeyCode::Home => app.move_cursor_home(),
                        KeyCode::End => app.move_cursor_end(),
                        _ => {}
                    }
                }
            }
        }

        if let Some(new_alias) = app.pending_alias_change.take() {
            let _ = storage.save_alias(&new_alias);
        }

        if app.pending_post {
            if let Some(msg) = app.timeline.first() {
                let _ = storage.save_message(msg);
            }
            app.pending_post = false;
        }

        if let Some(post_id) = app.pending_nod.take() {
            if let Some(msg) = app.timeline.iter().find(|m| m.id == post_id) {
                let _ = storage.save_message(msg);
            }
        }

        if let Some((post_id, add)) = app.pending_bookmark.take() {
            if add {
                let _ = storage.bookmark_post(&post_id);
            } else {
                let _ = storage.unbookmark_post(&post_id);
            }
        }

        for id in app.pending_deletes.drain(..) {
            let _ = storage.delete_message(&id);
            let _ = storage.unbookmark_post(&id);
        }

        if app.pending_save {
            for msg in &app.timeline {
                let _ = storage.save_message(msg);
            }
            app.pending_save = false;
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
