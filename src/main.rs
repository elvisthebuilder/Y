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
#[command(name = "y", about = "Decentralized, anonymous chat over Tor", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Open Y — launch the chat interface
    Open,
    /// Run Y as a headless mediator node (no TUI)
    Serve,
    /// Update Y to the latest release
    Update,
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
        Some(Command::Serve) => serve().await,
        Some(Command::Update) => update(),
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

fn update() -> Result<()> {
    use std::process::Command as Cmd;

    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: v{}", current_version);
    println!("Checking for updates...");

    let output = Cmd::new("curl")
        .args([
            "-sL",
            "https://api.github.com/repos/elvisthebuilder/Y/releases/latest",
        ])
        .output()?;

    let body = String::from_utf8_lossy(&output.stdout);
    let latest_tag = body
        .lines()
        .find(|l| l.contains("\"tag_name\""))
        .and_then(|l| {
            let after_key = &l[l.find("tag_name")? + 10..];
            let start = after_key.find('"')? + 1;
            let end = start + after_key[start..].find('"')?;
            Some(after_key[start..end].to_string())
        });

    let latest = match latest_tag {
        Some(t) => t,
        None => {
            println!("Could not fetch latest release.");
            return Ok(());
        }
    };

    let latest_version = latest.trim_start_matches('v');
    if latest_version == current_version {
        println!("Already up to date (v{}).", current_version);
        return Ok(());
    }

    println!(
        "New version available: {} (current: v{})",
        latest, current_version
    );

    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let archive = match (os, arch) {
        ("linux", "x86_64") => "y-linux-x86_64.tar.gz",
        ("macos", "x86_64") => "y-macos-x86_64.tar.gz",
        ("macos", "aarch64") => "y-macos-aarch64.tar.gz",
        _ => {
            println!(
                "Unsupported platform ({} {}). Download manually from:",
                os, arch
            );
            println!(
                "  https://github.com/elvisthebuilder/Y/releases/tag/{}",
                latest
            );
            return Ok(());
        }
    };

    let url = format!(
        "https://github.com/elvisthebuilder/Y/releases/download/{}/{}",
        latest, archive
    );

    println!("Downloading {}...", archive);

    let tmp_dir = std::env::temp_dir().join("y-update");
    let _ = std::fs::create_dir_all(&tmp_dir);
    let archive_path = tmp_dir.join(archive);

    let dl = Cmd::new("curl")
        .args(["-sL", &url, "-o"])
        .arg(&archive_path)
        .status()?;

    if !dl.success() {
        println!("Download failed.");
        return Ok(());
    }

    println!("Extracting...");
    let extract = Cmd::new("tar")
        .args(["xzf"])
        .arg(&archive_path)
        .arg("-C")
        .arg(&tmp_dir)
        .status()?;

    if !extract.success() {
        println!("Extraction failed.");
        return Ok(());
    }

    let new_binary = tmp_dir.join("y");
    let current_exe = std::env::current_exe()?;

    println!("Updating {}...", current_exe.display());

    let status = Cmd::new("cp").arg(&new_binary).arg(&current_exe).status()?;

    let status = if !status.success() {
        println!("Retrying with sudo...");
        Cmd::new("sudo")
            .args(["cp"])
            .arg(&new_binary)
            .arg(&current_exe)
            .status()?
    } else {
        status
    };

    let _ = std::fs::remove_dir_all(&tmp_dir);

    if status.success() {
        println!("Updated to {}.", latest);
    } else {
        println!("Update failed. Try running with sudo: sudo y update");
    }

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

    let engine_discovery = Arc::clone(&engine);
    tokio::spawn(async move {
        engine_discovery.run_discovery_loop().await;
    });

    let engine_seed = Arc::clone(&engine);
    tokio::spawn(async move {
        engine_seed.connect_to_seeds().await;
    });

    let engine_health = Arc::clone(&engine);
    tokio::spawn(async move {
        engine_health.run_health_check_loop().await;
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
                        KeyCode::Up => app.handle_arrow_up(),
                        KeyCode::Down => app.handle_arrow_down(),
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
                NetworkEvent::PeerConnected { alias, address } => {
                    app.add_known_user(alias.clone(), address);
                    app.status_message = format!("Peer connected: {}", alias);
                }
                NetworkEvent::PeerDisconnected { address } => {
                    app.status_message = format!("Peer disconnected: {}", address);
                }
                NetworkEvent::NewDirectMessage(envelope) => {
                    let sender_alias = app
                        .known_users
                        .iter()
                        .find(|(_, a)| *a == envelope.sender)
                        .map(|(alias, _)| alias.clone())
                        .unwrap_or_else(|| "unknown".to_string());
                    let payload = crate::crypto::encryption::EncryptedPayload {
                        ephemeral_public: envelope.ephemeral_public,
                        nonce: envelope.nonce,
                        ciphertext: envelope.ciphertext,
                    };
                    let x25519_secret = identity.x25519_secret();
                    let text = match crate::crypto::encryption::decrypt_payload(
                        &payload,
                        &x25519_secret,
                    ) {
                        Ok(plaintext) => String::from_utf8_lossy(&plaintext).to_string(),
                        Err(_) => {
                            app.status_message =
                                format!("Failed to decrypt DM from {}", sender_alias);
                            continue;
                        }
                    };
                    app.receive_dm(envelope.sender.clone(), sender_alias.clone(), text);
                    app.status_message = format!("New DM from {}", sender_alias);
                }
                NetworkEvent::OnionReady(addr) => {
                    app.status_message = format!("Tor hidden service ready: {}", addr);
                    app.onion_address = Some(addr);
                    app.is_online = true;
                }
                NetworkEvent::ConnectivityChanged(online) => {
                    app.is_online = online;
                    app.status_message = if online {
                        "Back online — synced".to_string()
                    } else {
                        "Offline — waiting for connection".to_string()
                    };
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

        if let Some((recipient_address, text)) = app.pending_dm.take() {
            let engine_dm = Arc::clone(&engine);
            let sender = identity.address.clone();
            tokio::spawn(async move {
                let vk_bytes = engine_dm.peer_verifying_key(&recipient_address).await;
                let envelope = if let Some(vk_bytes) = vk_bytes {
                    let vk = ed25519_dalek::VerifyingKey::from_bytes(&vk_bytes).ok();
                    let x25519_pub =
                        vk.and_then(|k| crate::crypto::identity::verifying_key_to_x25519(&k));
                    if let Some(pub_key) = x25519_pub {
                        match crate::crypto::encryption::encrypt_for_recipient(
                            text.as_bytes(),
                            &pub_key,
                        ) {
                            Ok(payload) => crate::network::protocol::EncryptedEnvelope {
                                recipient: recipient_address,
                                sender,
                                ephemeral_public: payload.ephemeral_public,
                                nonce: payload.nonce,
                                ciphertext: payload.ciphertext,
                                timestamp: chrono::Utc::now(),
                            },
                            Err(_) => return,
                        }
                    } else {
                        return;
                    }
                } else {
                    return;
                };
                let _ = engine_dm.send_dm(envelope).await;
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

async fn serve() -> Result<()> {
    tracing_subscriber::fmt()
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
            a
        }
    };

    let listen_port = std::env::var("Y_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7331u16);

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let engine = Arc::new(NetworkEngine::new(
        identity.clone(),
        user_alias,
        listen_port,
        data_path,
        event_tx,
    ));

    println!("Y mediator starting...");
    println!("Identity: {}", identity.address);

    let engine_handle = Arc::clone(&engine);
    tokio::spawn(async move {
        if let Err(e) = engine_handle.start().await {
            tracing::error!("Network engine error: {}", e);
        }
    });

    let engine_discovery = Arc::clone(&engine);
    tokio::spawn(async move {
        engine_discovery.run_discovery_loop().await;
    });

    let engine_seed = Arc::clone(&engine);
    tokio::spawn(async move {
        engine_seed.connect_to_seeds().await;
    });

    let engine_health = Arc::clone(&engine);
    tokio::spawn(async move {
        engine_health.run_health_check_loop().await;
    });

    println!("Waiting for Tor hidden service...");

    loop {
        while let Ok(event) = event_rx.try_recv() {
            match event {
                NetworkEvent::OnionReady(addr) => {
                    println!();
                    println!("========================================");
                    println!("  THE MEDIATOR — ONLINE");
                    println!("  Address: {}", addr);
                    println!("========================================");
                    println!();
                    println!("Add this to SEED_NODES or use:");
                    println!("  Y_SEEDS={} y open", addr);
                    println!();
                }
                NetworkEvent::PeerConnected { alias, address } => {
                    println!("[+] Peer connected: {} ({})", alias, address);
                }
                NetworkEvent::PeerDisconnected { address } => {
                    println!("[-] Peer disconnected: {}", address);
                }
                NetworkEvent::PeerCountChanged(count) => {
                    println!("[*] Active peers: {}", count);
                }
                NetworkEvent::NewPost(msg) => {
                    let _ = storage.save_message(&msg);
                }
                NetworkEvent::ConnectivityChanged(online) => {
                    if online {
                        println!("[*] Connectivity: ONLINE");
                    } else {
                        println!("[!] Connectivity: OFFLINE");
                    }
                }
                _ => {}
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
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
