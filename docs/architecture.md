# Architecture

## Overview

Y is built around a decentralized, peer-to-peer architecture where every instance acts as both a client and a network participant. Instead of relying on centralized infrastructure, each node maintains its own identity, communicates directly with peers over the Tor network, stores local data, and participates in message propagation and distributed storage.

The application is organized into modular components responsible for cryptography, networking, storage, communities, and the terminal user interface.

---

## High-Level Architecture

```text
                 +----------------------+
                 |     Terminal UI      |
                 +----------+-----------+
                            |
                            v
                 +----------------------+
                 |    Network Engine    |
                 +----------+-----------+
                            |
        +-------------------+-------------------+
        |                   |                   |
        v                   v                   v
+---------------+   +----------------+   +---------------+
|    Crypto     |   |    Protocol    |   |    Storage    |
+---------------+   +----------------+   +---------------+
        |                   |                   |
        |                   |                   |
        |            +------+-------+           |
        |            |              |           |
        |            v              v           |
        |        Tor Transport   Kademlia DHT  |
        |            |              |           |
        +------------+------+-------+-----------+
                             |
                             v
                        Other Y Peers
```

---

## Identity

Every user owns a cryptographic identity that is generated locally during the first launch.

The identity is based on an Ed25519 keypair used to sign messages and derive the user's permanent address. Since identities are never managed by a central authority, users remain in full control of their credentials and can authenticate messages without relying on external services.

---

## Networking

Y communicates exclusively over Tor hidden services. Each node exposes a `.onion` address, allowing peers to establish connections without revealing their public IP addresses.

The networking layer is managed by the `NetworkEngine`, which is responsible for peer connections, message propagation, peer discovery, direct message routing, and coordination with the distributed storage layer.

---

## Wire Protocol

Communication between peers is performed through serialized `WireMessage` packets.

The protocol includes message types for:

* Peer handshake (`Hello`, `HelloAck`)
* Public timeline synchronization
* Gossip propagation of posts
* End-to-end encrypted direct messages
* Peer discovery
* DHT requests and responses
* Keep-alive (`Ping` / `Pong`)
* Notification events such as nod propagation

This message-oriented design keeps communication structured while allowing new protocol features to be added without changing the transport layer.

---

## Gossip Protocol

Public posts are propagated using a gossip-based protocol.

Instead of forwarding every message through a central relay, peers exchange newly received posts with connected neighbors. Each receiving peer verifies the message before propagating it further, allowing content to spread throughout the network while avoiding single points of failure.

---

## Distributed Storage

Y uses a Kademlia-based Distributed Hash Table (DHT) to distribute and replicate data across participating nodes.

The DHT is responsible for storing and retrieving shared network data, allowing content such as posts and encrypted direct messages to remain available even when the original sender is temporarily offline.

---

## Local Storage

Local persistence is provided by the embedded `sled` database.

Each node stores its own application data locally, including information such as:

* Cryptographic identity
* User alias
* Cached messages
* Bookmarks
* Timeline data

Because storage is fully embedded, Y does not require an external database server.

---

## Terminal User Interface

Y provides a terminal-based user interface (TUI) built around an event-driven architecture.

The interface handles user interaction, rendering, keyboard shortcuts, and navigation while the networking components continue processing peer communication in the background. This separation keeps the interface responsive during synchronization and message exchange.

---

## Design Goals

The architecture is designed around several core principles:

* Decentralization without centralized infrastructure.
* Cryptographic identity instead of account-based authentication.
* Privacy through Tor hidden services.
* Distributed message propagation using gossip.
* Resilient storage through a Kademlia DHT.
* Local ownership of user data.
* Modular components with clearly separated responsibilities.
