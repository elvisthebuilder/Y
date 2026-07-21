# Y

![Mediators](https://img.shields.io/badge/mediators-1-brightgreen) ![1000 posts](https://img.shields.io/badge/1000_posts-1-blue)

*Active seed nodes and their timeline capacity.*

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
- Each user runs a **Tor hidden service** (via arti-client)
- Your real IP is never exposed — all traffic routed through Tor
- Peers connect via `.onion` addresses
- Messages gossip through the network — no central relay
- **Kademlia DHT** distributes and replicates content across nodes
- Posts persist even when the author is offline
- DMs are stored encrypted at the recipient's DHT key until retrieved
- Nothing to take down, nothing to subpoena

### Communities
- Open (anyone can join) or private (owner approval required)
- Owner-controlled membership with interactive approve/decline flow
- Join requests queue up for private communities — owners navigate and act on each
- No moderation from above — communities self-govern

## Install & Run

### One-liner (Linux / macOS)

```bash
curl -sL https://raw.githubusercontent.com/elvisthebuilder/Y/main/install.sh | sh
```

### Native installers

Download from [Releases](https://github.com/elvisthebuilder/Y/releases):

| Platform | Format | Install |
|----------|--------|---------|
| Linux | `.deb` | `sudo dpkg -i y_*.deb` |
| macOS | `.dmg` | Open, drag to Applications |
| Windows | `.msi` | Double-click to install |

After install:

```bash
y open
```

Running `y` with no subcommand also opens the chat interface.

### Build from source

```bash
git clone https://github.com/elvisthebuilder/Y.git
cd Y
cargo install --path .
```

On first run, Y bootstraps the Tor client and creates your hidden service. This takes ~30 seconds the first time (downloading Tor consensus data), subsequent launches are faster.

### The Mediator (peer discovery)

Y ships with a default seed node called **the mediator**. On startup, your client connects to it automatically and discovers other peers through the DHT. No manual peer configuration needed.

The mediator has no special privileges — it can't read your DMs, censor posts, or control the network. It's just a peer that's always online so new users can find everyone else. If it goes down, existing peers continue talking to each other.

You can override or add additional seed nodes:

```bash
Y_SEEDS=your-seed.onion:7331,another-seed.onion:7331 y open
```

Or connect to a specific peer directly:

```bash
Y_PEER=someone.onion:7331 y open
```

### Run your own seed node

Anyone can run a seed node. The more seeds, the more resilient the network.

```bash
# On any always-on server (EC2, VPS, Raspberry Pi, etc.)
y serve
```

This runs the network engine headless — no TUI, just peer connections. It prints the `.onion` address on startup. Submit it as a PR to `SEED_NODES` in `src/network/engine.rs` to help bootstrap the network for everyone.

```bash
# Custom port
Y_PORT=8080 y serve

# Limit stored posts (default: 1000)
y serve --max-posts 500
```

Anyone can contribute seed nodes by running `y serve` and opening a PR. The more seeds, the less dependent the network is on any single one.

### Update

```bash
y update
```

Checks GitHub for the latest release and replaces the current binary in-place. Falls back to `sudo` if needed.

### Reset

```bash
y reset
```

Clears your timeline, bookmarks, and cached data while keeping your identity and alias intact. Useful after major updates to start fresh without losing who you are.

To also generate a new identity and alias:

```bash
y reset --new-identity
```

### Uninstall

```bash
y uninstall
```

This removes the binary and all local data (`~/.root-chat`).

## Controls

| Key | Action |
|-----|--------|
| `t` | Timeline (public posts) |
| `d` | Direct messages |
| `c` | Communities |
| `b` | Bookmarks |
| `p` | Profile / identity |
| `n` | Compose new post |
| `j`/`k` | Navigate posts |
| `.` | Nod at selected post (respect) |
| `r` | Reply to selected post |
| `s` | Bookmark / unbookmark post |
| `x` | Delete your post |
| `g` | Go to post (from bookmarks) |
| `a` | Approve pending request (community detail) |
| `x` | Decline pending request (community detail) |
| `Enter` | Expand/collapse replies, open community |
| `Shift+Enter` | New line while composing |
| `/` | Search users |
| `:` | Command mode |
| `y` | Copy onion address (in Profile) |

### Commands
- `:whoami` — Show your handle and address
- `:peers` — Show connected peer count
- `:alias <name>` — Set your alias manually
- `:alias-gen` — Generate a random alias
- `:search <query>` — Search users by alias or address
- `:create <name>` — Create an open community
- `:create <name> private` — Create a private community (approval required)
- `:join <id>` — Join a community (or request to join if private)
- `:quit` / `:q` — Exit

## Identity & Aliases

Your identity is auto-generated on first run:
- **Address**: `root:a8Kx2m...` (cryptographic, permanent)
- **Alias**: `phantom-cipher` (human-readable, changeable)
- **Handle**: `phantom-cipher#a8Kx` (alias + short address for disambiguation)

Aliases are not globally unique — multiple users can share the same alias. The `#shortcode` suffix disambiguates them. Set yours manually or let Y generate one for you.

## Interactions

- **Nod** — A subtle acknowledgment. No hearts, no clapping. Just respect. One nod per user per post.
- **Reply** — Threaded replies linked to the parent post. Open any post to see the full thread.
- **Bookmark** — Save posts locally. Only you can see your bookmarks.

## Architecture

```
src/
├── main.rs              — Entry point, TUI event loop, network integration
├── crypto/
│   ├── identity.rs      — Ed25519 keypair, address derivation, signing
│   ├── alias.rs         — Alias generation and display handles
│   └── encryption.rs    — X25519 Diffie-Hellman + ChaCha20Poly1305
├── protocol/
│   └── message.rs       — Message types, nods, replies
├── network/
│   ├── tor.rs           — Tor hidden service (arti-client)
│   ├── engine.rs        — Connection management, gossip, DM routing
│   ├── codec.rs         — Length-prefixed framing over Tor streams
│   ├── protocol.rs      — Wire message types (Hello, DHT, gossip)
│   ├── dht.rs           — Kademlia DHT (routing table, storage, replication)
│   ├── peer.rs          — Peer registry
│   └── search.rs        — User search
├── storage/
│   └── mod.rs           — sled embedded DB (identity, messages, bookmarks)
├── community/
│   └── mod.rs           — Community membership and access control
└── tui/
    ├── app.rs           — App state, keybindings, threaded view
    └── ui.rs            — Terminal UI rendering (tree lines, scrolling)
```

## Design Principles

1. **Zero trust** — No central authority. No server to compromise.
2. **Identity is cryptography** — Not an email. Not a username in someone else's database.
3. **Uncensorable** — Posts are signed and gossiped. No one can delete what you said.
4. **Private by default** — DMs are end-to-end encrypted. Tor hides your location.
5. **Self-sovereign** — Your keys, your data, your rules.

## Roadmap

- [x] Tor hidden service integration (arti-client)
- [x] Gossip protocol for post propagation
- [x] Kademlia DHT for distributed storage
- [x] End-to-end encrypted DMs with store-and-forward
- [x] Threaded replies with collapse/expand
- [x] Alias system with disambiguation
- [ ] Media attachments (encrypted)
- [x] Community creation, join requests, and owner approval flow
- [x] The mediator — seed node for automatic peer discovery
- [ ] Mobile client
- [ ] Onion-routed file sharing

## License

MIT
