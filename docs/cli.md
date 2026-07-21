# CLI Reference

## Overview

Y provides a small set of command-line subcommands for launching the application, running infrastructure nodes, updating the installation, and removing local data.

If no subcommand is provided, running `y` starts the chat interface.

---

# Commands

## `y`

Starts the interactive terminal interface.

Equivalent to:

```bash
y open
```

---

## `y open`

Launches the terminal user interface.

During startup Y:

* Loads or creates your identity.
* Opens the local database.
* Starts the Tor client.
* Creates a hidden service.
* Connects to configured peers.
* Opens the chat interface.

---

## `y serve`

Runs Y without the terminal interface.

This mode is intended for always-on machines acting as seed nodes for peer discovery.

Example:

```bash
y serve
```

---

## `y update`

Checks for the latest available release and updates the installed binary if a newer version is available.

Example:

```bash
y update
```

---

## `y reset`

Removes local application data including:

- Timeline
- Bookmarks
- Cache

Your identity is preserved. Use `--new-identity` to generate a new one.

Example:

```bash
y reset
```

To generate a completely new identity as well:

```bash
y reset --new-identity
```

---

## `y uninstall`

Removes the installed binary together with local application data.

Example:

```bash
y uninstall
```

---

# Command Mode

Inside the application, press `:` to enter command mode.

Available commands include:

| Command | Description |
|---------|-------------|
| `:whoami` | Display your identity and handle |
| `:peers` | Show the number of connected peers |
| `:alias <name>` | Set a custom alias |
| `:alias-gen` | Generate a random alias |
| `:search <query>` | Search users |
| `:create <name>` | Create a public community |
| `:create <name> private` | Create a private community |
| `:join <id>` | Join a community |
| `:q` | Exit the application |
| `:quit` | Exit the application |

---

# Environment Variables

Y supports several optional environment variables for configuring startup behavior.

| Variable     | Description                               |
| ------------ | ----------------------------------------- |
| `Y_DATA_DIR` | Override the default local data directory |
| `Y_SEEDS`    | Specify one or more seed nodes            |
| `Y_PEER`     | Connect directly to a specific peer       |
| `Y_PORT`     | Change the listening port                 |

Example:

```bash
Y_PORT=8080 Y_DATA_DIR=~/.root-chat-dev y open
```

---

# Keyboard Shortcuts

The terminal interface is keyboard-driven.

Some commonly used shortcuts include:

| Key | Action |
| --- | ------ |
| `t` | Timeline |
| `d` | Direct messages |
| `c` | Communities |
| `b` | Bookmarks |
| `p` | Profile |
| `n` | Compose a new post |
| `.` | Nod / Like the selected post |
| `s` | Bookmark the selected post |
| `r` | Reply to the selected post |
| `x` | Delete your own selected post |
| `/` | Search for users |
| `i` | Start typing in the selected DM conversation |
| `j` | Move to the next item |
| `k` | Move to the previous item |
| `g` | Open the original post from Bookmarks |
| `Enter` | Open the selected item or expand/collapse a thread |


