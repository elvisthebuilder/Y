# Y

> No servers. No accounts. No censorship. Your keys = your identity. Tor = your shield.

Y is a decentralized, anonymous chat platform built in Rust. It runs over the Tor network with cryptographic identity — no signups, no emails, no phone numbers. You are your keypair.

## How It Works

```
You type → Sign (Ed25519) → Encrypt (if DM) → Send via Tor → Peers verify → Display
```

### Identity
- First run generates an Ed25519 keypair
- Your address: `root:a8Kx2m...` (derived from your public key)
- Stored locally at `~/.root-chat/db`
- No registration. No server. You *are* your keys.

### Messages
| Type | Visibility | Encryption |
|------|-----------|------------|
| Post | Public broadcast | Signed, not encrypted |
| DM | Sender + recipient only | X25519 + ChaCha20Poly1305 |
| Community | Group members | Signed, group-scoped |

Every message is cryptographically signed — tampered messages are rejected by peers.

### Network
- Each user runs a **Tor hidden service**
- Your real IP is never exposed
- Peers connect via `.onion` addresses
- Messages gossip through the network — no central relay
- Nothing to take down, nothing to subpoena

### Communities
- Open (anyone can join) or locked (invite-only)
- Owner-controlled membership
- No moderation from above — communities self-govern

## Install & Run

```bash
# Clone and build
git clone <repo-url>
cd root-chat-software
cargo build --release

# Run
cargo run
```

## Controls

| Key | Action |
|-----|--------|
| `t` | Timeline (public posts) |
| `d` | Direct messages |
| `c` | Communities |
| `p` | Profile / identity |
| `n` | Compose new post |
| `j`/`k` | Scroll down/up |
| `:` | Command mode |
| `q` | Quit |

### Commands
- `:whoami` — Show your address
- `:peers` — Show connected peer count
- `:quit` — Exit

## Architecture

```
src/
├── main.rs              — Entry point, TUI event loop
├── crypto/
│   ├── identity.rs      — Ed25519 keypair, address derivation, signing
│   └── encryption.rs    — X25519 Diffie-Hellman + ChaCha20Poly1305
├── protocol/
│   ├── message.rs       — Message types, peer commands
│   └── commands.rs      — Wire protocol envelope
├── network/
│   ├── tor.rs           — Tor hidden service management
│   └── peer.rs          — Peer registry and discovery
├── storage/
│   └── mod.rs           — sled embedded DB
├── community/
│   └── mod.rs           — Community membership and access control
└── tui/
    ├── app.rs           — App state, keybindings
    └── ui.rs            — Terminal UI rendering
```

## Design Principles

1. **Zero trust** — No central authority. No server to compromise.
2. **Identity is cryptography** — Not an email. Not a username in someone else's database.
3. **Uncensorable** — Posts are signed and gossiped. No one can delete what you said.
4. **Private by default** — DMs are end-to-end encrypted. Tor hides your location.
5. **Self-sovereign** — Your keys, your data, your rules.

## Roadmap

- [ ] Full Tor hidden service integration (via arti)
- [ ] Gossip protocol for post propagation
- [ ] Peer discovery via DHT
- [ ] Media attachments (encrypted)
- [ ] Community invites and moderation tools
- [ ] Mobile client
- [ ] Onion-routed file sharing

## License

MIT
