# Contributing to Y

Thanks for your interest in contributing to Y. This project is decentralized by design, and contributions help keep it that way.

## Getting Started

```bash
git clone https://github.com/elvisthebuilder/Y.git
cd Y
cargo build
```

You'll need Rust (stable) and a C toolchain for the SQLite dependency.

## Running Locally

```bash
# Run the TUI
cargo run -- open

# Run with a separate database (useful for testing alongside production)
Y_DATA_DIR=~/.root-chat-dev cargo run -- open

# Run headless (seed node mode)
cargo run -- serve
```

## Before Submitting a PR

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

All three must pass. CI enforces this.

## What to Work On

Check the [issues](https://github.com/elvisthebuilder/Y/issues) for open tasks. Some areas that could use help:

- **Seed nodes** — Run `y serve` on an always-on server and submit your `.onion` address to `SEED_NODES` in `src/network/engine.rs`. The more seeds, the more resilient the network.
- **Platform support** — Testing and fixes for macOS, Windows, and ARM Linux.
- **Network resilience** — Peer reconnection, gossip protocol improvements, DHT replication tuning.
- **TUI improvements** — New views, better navigation, accessibility.
- **Documentation** — Usage guides, architecture docs, protocol specs.

## Project Structure

```
src/
├── main.rs           — Entry point, CLI subcommands, TUI event loop
├── crypto/           — Identity (Ed25519), encryption (X25519 + ChaCha20), aliases
├── protocol/         — Message types, nods, replies
├── network/          — Tor transport, gossip engine, DHT, peer management
├── storage/          — sled embedded database
├── community/        — Community membership and access control
└── tui/              — Terminal UI (app state, rendering)
```

## Guidelines

- Keep PRs focused. One feature or fix per PR.
- No unnecessary abstractions. Three similar lines is better than a premature helper.
- Don't introduce external network calls, tracking, or telemetry. Y is private by design.
- Test your changes against a running instance, not just `cargo check`.
- Commit messages should explain *why*, not *what*.

## Running a Seed Node

If you have a server (VPS, EC2, Raspberry Pi, anything always-on):

```bash
y serve
```

It prints your `.onion` address on startup. Open a PR adding it to the `SEED_NODES` array in `src/network/engine.rs`. Every seed makes the network stronger.

## Security

If you find a security vulnerability, please report it privately to elvisthebuilder rather than opening a public issue. Responsible disclosure is appreciated.

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
