#![allow(dead_code)]

mod community;
mod crypto;
mod network;
mod protocol;
mod storage;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

use crate::crypto::alias;
use crate::crypto::identity::Identity;
use crate::network::engine::{NetworkEngine, NetworkEvent};
use crate::storage::Storage;
use crate::tui::app::App;

#[derive(Parser)]
#[command(name = "y", about = "Decentralized, anonymous chat over Tor")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Open Y — launch the chat interface
    Open,
    /// Uninstall Y — remove binary and data
    Uninstall,
}

fn data_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("Y_DATA_DIR") {
        return PathBuf::from(custom);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".root-chat")
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Uninstall) => uninstall(),
        Some(Command::Open) | None => open().await,
    }
}

fn uninstall() -> Result<()> {
    let data_path = data_dir();

    // Remove data directory
    if data_path.exists() {
        std::fs::remove_dir_all(&data_path)?;
        println!("Removed data directory: {}", data_path.display());
    } else {
        println!("No data directory found at {}", data_path.display());
    }

    // Remove the binary
    if let Ok(exe) = std::env::current_exe() {
        std::fs::remove_file(&exe)?;
        println!("Removed binary: {}", exe.display());
    }

    println!("Y has been uninstalled.");
    Ok(())
}

async fn open() -> Result<()> {
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

    let listen_port = std::env::var("Y_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7331u16);

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let engine = Arc::new(NetworkEngine::new(
        identity.clone(),
        user_alias.clone(),
        listen_port,
        data_path.clone(),
        event_tx,
    ));

    let engine_handle = Arc::clone(&engine);
    tokio::spawn(async move {
        if let Err(e) = engine_handle.start().await {
            tracing::error!("Network engine error: {}", e);
        }
    });

    if let Ok(peer) = std::env::var("Y_PEER") {
        let engine_connect = Arc::clone(&engine);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            if let Err(e) = engine_connect.connect_to(&peer).await {
                tracing::warn!("Failed to connect to peer {}: {}", peer, e);
            }
        });
    }

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

        while let Ok(event) = event_rx.try_recv() {
            match event {
                NetworkEvent::NewPost(msg) => {
                    let _ = storage.save_message(&msg);
                    app.timeline.insert(0, msg);
                }
                NetworkEvent::NodReceived { post_id, from } => {
                    if let Some(msg) = app.timeline.iter_mut().find(|m| m.id == post_id) {
                        let nod = crate::protocol::message::Nod {
                            from: from.clone(),
                            timestamp: chrono::Utc::now(),
                        };
                        msg.nods.push(nod);
                        let _ = storage.save_message(msg);
                    }
                }
                NetworkEvent::PeerCountChanged(count) => {
                    app.peer_count = count;
                }
                NetworkEvent::PeerConnected { alias, .. } => {
                    app.status_message = format!("Peer connected: {}", alias);
                }
                NetworkEvent::PeerDisconnected { address } => {
                    app.status_message = format!("Peer disconnected: {}", address);
                }
                NetworkEvent::NewDirectMessage(_envelope) => {
                    app.status_message = "New encrypted DM received".to_string();
                }
                NetworkEvent::OnionReady(addr) => {
                    app.status_message = format!("Tor hidden service ready: {}", addr);
                    app.onion_address = Some(addr);
                }
            }
        }

        if let Some(text) = app.pending_copy.take() {
            copy_to_clipboard(&text);
        }

        if let Some(new_alias) = app.pending_alias_change.take() {
            let _ = storage.save_alias(&new_alias);
        }

        if app.pending_post {
            if let Some(msg) = app.timeline.first() {
                let _ = storage.save_message(msg);
                let engine_bc = Arc::clone(&engine);
                let msg_clone = msg.clone();
                tokio::spawn(async move {
                    let _ = engine_bc.broadcast_post(&msg_clone).await;
                });
            }
            app.pending_post = false;
        }

        if let Some(post_id) = app.pending_nod.take() {
            if let Some(msg) = app.timeline.iter().find(|m| m.id == post_id) {
                let _ = storage.save_message(msg);
            }
            let engine_nod = Arc::clone(&engine);
            let nod_id = post_id.clone();
            tokio::spawn(async move {
                let _ = engine_nod.broadcast_nod(&nod_id).await;
            });
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

    println!("Y terminated. Identity: {}", identity.address);
    Ok(())
}

fn copy_to_clipboard(text: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let commands = [
        ("xclip", vec!["-selection", "clipboard"]),
        ("xsel", vec!["--clipboard", "--input"]),
        ("wl-copy", vec![]),
    ];

    for (cmd, args) in &commands {
        if let Ok(mut child) = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
            return;
        }
    }
}
