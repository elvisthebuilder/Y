# Usage Guide

## Overview

Y is a decentralized chat platform that operates without centralized servers or user accounts. Instead of creating an account with an email address or phone number, your identity is created from a cryptographic keypair generated on your device.

Communication takes place over the Tor network, helping keep your network location private while allowing peers to communicate directly. Public posts, encrypted direct messages, and communities are all built on the same decentralized network.

This guide walks through the most common tasks you'll perform when using Y.

---

## First Launch

Start Y by running:

```bash
y open
```

On the first launch, Y performs several setup steps automatically:

* Generates your cryptographic identity.
* Creates a local data directory.
* Boots the embedded Tor client.
* Starts your hidden service.
* Connects to available peers through the configured seed nodes.

The first startup may take longer than later launches because Tor needs to download its initial network information.

Once initialization is complete, the terminal interface opens and you're ready to interact with the network.

---

## Your Identity

Every installation has its own identity consisting of three parts:

### Address

Your permanent cryptographic identifier, derived from your public key.

Example:

```
root:a8Kx2m...
```

This address uniquely identifies you across the network.

### Alias

A human-friendly display name.

Aliases can be changed at any time and are **not unique**.

### Handle

The handle combines your alias with a shortened version of your address.

Example:

```
phantom-cipher#a8Kx
```

This makes it easier to distinguish users with the same alias.

Never share your private keys with anyone. Your identity belongs entirely to you.

---

## Navigating the Interface

The terminal interface is divided into multiple sections that let you browse different parts of the network.

Common views include:

* Timeline
* Direct Messages
* Communities
* Bookmarks
* Profile

Navigate using the documented keyboard shortcuts and switch between views without leaving the application.

---

## Creating Your First Post

To publish a public message:

1. Open the Timeline.
2. Press the compose shortcut.
3. Write your message.
4. Submit it.

Every public post is digitally signed before being broadcast through the network. Other peers verify the signature before accepting the message.

---

## Sending Direct Messages

Direct messages are encrypted before leaving your device.

Only the intended recipient can decrypt the message contents.

If the recipient is temporarily offline, encrypted messages remain available through the distributed network until they are retrieved.

---

## Communities

Communities allow discussions around shared interests.

Y supports two community types:

* Open communities that anyone can join.
* Private communities that require approval from the owner.

Community owners manage membership requests directly without relying on centralized moderation.

---

## Searching for Users

Open command mode and use the search command to locate users by alias or address.

If multiple users share the same alias, compare their handles to identify the correct person.

---

## Bookmarks

Bookmarks let you save interesting posts locally for future reference.

Bookmarks exist only on your device and are not shared with other peers.

---

## Running a Seed Node

If you operate an always-on machine such as a VPS or Raspberry Pi, you can run Y in headless mode:

```bash
y serve
```

Seed nodes help new peers discover the network and improve overall resilience. They do not receive special privileges or control the network.

---

## Tips

* Keep your private keys secure.
* Use aliases to make conversations easier to follow.
* Leave a seed node running if you have a reliable server.
* Keep Y updated to receive protocol improvements and bug fixes.
* Review the CLI reference for additional commands and configuration options.
