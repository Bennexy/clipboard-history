# Clipstash

A lightweight, high-performance clipboard history manager for Linux, written entirely in Rust.
It focuses on speed, privacy, and user control by keeping clipboard data local and providing a lightweight daemon-based architecture.

Clipstash is designed around a simple idea: **keep the clipboard daemon running, and make the UI appear instantly whenever it's needed.**

Instead of combining clipboard monitoring and the user interface into a single application, Clipstash separates them into independent components that communicate over a Unix domain socket.

* **clipd** – a background daemon that continuously monitors the system clipboard, stores clipboard history, and serves connected clients.
* **clipui** – a lightweight popup interface built with **egui** for searching and selecting clipboard entries.
* **clip-launcher** – a small helper that requests the daemon to display the UI and (planned) automatically starts it if it is not already running.

This architecture keeps resource usage low while minimizing popup latency.

> !Disclamer!
> Ai was used in the development of this application. AI usage was limited to information gathering and reviews. The Architecture and Code was designed and written by me. This README is mainly AI generated. I created this project as a recreational-project since i enjoy the process of creating, i do not enjoy "vibe-coding" as it lacks what i love about coding. Creativity. That said, enjoy the app!

---
Clipstash is free and open-source software licensed under the GNU General Public License v3.0. 

See [LICENSE](LICENSE) for details.

The goal is to provide a transparent, privacy-focused clipboard manager where users remain in control of their data.

---

# Features

* 🚀 Fast popup UI
* ⚡ Persistent background daemon
* 🦀 Written entirely in Rust
* 🧠 SQLite-backed clipboard history
* 🔎 SQLite Full-Text Search (FTS5)
* 📋 Restore previous clipboard entries with a single click
* 🔄 Live daemon → UI updates
* 🔌 Event-driven Unix domain socket communication
* 🎯 Lightweight multi-process architecture
* 🖥️ X11 support with Wayland compatibility
* 🔒 Privacy focused

## Privacy

Clipstash is designed with privacy in mind:

- Clipboard data stays local
- No telemetry
- No cloud synchronization
- No external services
- No tracking

The daemon only communicates with local clients through Unix domain sockets.

---

# Current Status

> **Early development**
>
> Clipstash is already functional but is still under active development. Features, APIs, and storage formats may change without notice.

### Currently implemented

* Clipboard monitoring
* SQLite-backed clipboard history
* Full-text search
* Fast popup UI
* Live daemon → UI communication
* Selecting an entry restores it to the system clipboard
* Automatic reconnection between the UI and daemon

### Currently in progress

* Pasting directly into the previously focused application
* Improved Wayland integration
* Better keyboard navigation
* Additional performance optimizations
* Packaging and binary releases

---

# Installation

There are currently no prebuilt binaries or Debian packages.

Rust and Cargo are required.

Clone the repository:

```bash
git clone https://github.com/Bennexy/clipboard-history
cd clipboard-history
```

Start the daemon:

```bash
cargo run -p clipd --release
```

Launch the UI manually:

```bash
cargo run -p clipui --release
```

Or use the launcher, which requests the daemon to display the popup UI:

```bash
cargo run -p clip-launcher --release
```

```text
    launcher
        │
        ▼
     ShowUi
        │
        ▼
      clipd
        │
        ▼
     running clipui
```

The UI can be started before or after the daemon. If the daemon is unavailable, the UI will automatically reconnect once it becomes available.

---

# Architecture

```text

+-------------------+   ShowUi     +----------------------+
|     launcher      | -----------> |        clipd         |
+-------------------+              |----------------------|
                                   | Clipboard watcher    |
                                   | SQLite + FTS         |
                                   | Socket server        |
                                   | Event broadcaster    |
                                   +----------+-----------+
                                              ^
                                              |
                                      Unix Domain Socket
                                              |
                                   +----------+-----------+
                                   |       clipui         |
                                   |   egui popup UI      |
                                   +----------------------+
```

## clipd

The daemon is responsible for:

* Monitoring clipboard changes
* Storing clipboard history
* Performing full-text searches
* Managing connected clients
* Broadcasting clipboard events
* Serving clipboard requests

## clipui

The UI is responsible for:

* Displaying clipboard history
* Searching clipboard entries
* Allowing clipboard selection
* Reacting to daemon events
* Presenting a lightweight popup interface

## clip-laucher

The Launcher is responsible for:

* Requesting visibility of the UI

---

# Storage

Clipboard history is stored in a SQLite database.

Current and planned storage modes include:

* Temporary on-disk database (is planed to be removed, easy dev work)
* In-memory database (planned)
* Persistent database location (planned)

The planned in-memory mode provides an ephemeral clipboard history that disappears after shutdown, making it useful for users who prefer not to leave clipboard data on disk.

---

# Design Goals

Clipstash aims to be:

* Fast (On average the socket responds in less than 0.5ms. Main time consumption is the UI)
* Lightweight
* Responsive
* Minimalistic
* Native to Linux
* Easy to maintain
* Fully written in Rust

Performance is the primary design goal.

The daemon remains running while the popup UI only connects when needed. This keeps startup latency extremely low while avoiding unnecessary background resource usage.

The daemon uses ~8mb of ram when running and > 0.1 cpu.
The ui uses ~120mb of ram and > 0.1 cpu.
The launcher is lightweight as well, i didnt measure it as its negitable at best.

---

# Roadmap

## Privacy
* [ ] only log messages in trace mode -> add explicit info to the config file that this will log all messages and message content!

## User Experience

* [x] Popup UI
* [x] Live daemon → UI updates
* [x] Clipboard restoration
* [ ] Paste directly into the previously focused application (only possible on X11)
* [ ] Better keyboard navigation (arrow keys, enter key for selection, etc.)
* [ ] Image clipboard support
* [ ] Thumbnail generation pipeline

## Performance

* [x] SQLite Full-Text Search
* [ ] In-memory SQLite storage
* [ ] Startup and rendering benchmarks
* [ ] Additional performance optimizations

## Reliability

* [x] Automatic socket reconnection
* [ ] Automatic UI startup when requested by the launcher
* [ ] Improved daemon robustness
* [ ] Better recovery from unexpected failures

## Configuration

* [ ] TOML configuration file
* [ ] Maximum clipboard history size
* [ ] Number of entries shown in the UI
* [ ] Configurable storage backend
* [ ] Ephemeral vs. persistent history
* [ ] User-configurable search behavior

## Platform Support

* [ ] Wayland polish
* [ ] Automatic startup (systemd user service)

## Releases

* [ ] Prebuilt binaries
* [ ] Debian package
* [ ] Installation documentation
* [ ] Benchmark and profiling documentation

---

# Contributing

Currently i would like to keep this a small private project as im far from being done and i am enjoying the learning process.
Bug Reports and ideas are welcome, Second or third party Contributions are currently not planed.