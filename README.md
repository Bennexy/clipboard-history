# Clipstash

A lightweight, high-performance clipboard history manager for Linux, written entirely in Rust.

Clipstash focuses on speed, privacy, and user control by keeping clipboard data local and using a lightweight daemon-based architecture.

Clipstash is designed around a simple idea:

> Keep the clipboard daemon running, and make the UI appear instantly whenever it is needed.

Instead of combining clipboard monitoring and the user interface into a single application, Clipstash separates them into independent components communicating through a Unix domain socket.

The project consists of:

* **clipd** – a background daemon that continuously monitors the system clipboard, stores clipboard history, and serves connected clients.
* **clipui** – a lightweight popup interface built with **egui** for searching and selecting clipboard entries.
* **clip-launcher** – a small helper that requests the daemon to display the UI and (planned) automatically starts it if it is not already running.

This architecture keeps resource usage low while minimizing popup latency.

---

> **Disclaimer**
>
> AI tools were used during development for information gathering, documentation assistance, and reviews.
>
> The architecture and implementation decisions are designed and written by the author. This README was created with AI assistance and manually reviewed.
>
> Clipstash is a recreational project created for the enjoyment of designing and building software. The goal is to learn, experiment, and create a useful application.

---

## License

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

The daemon does not transmit clipboard data outside the local machine.

Debug logging may expose clipboard contents and should only be enabled during development.

---

# Why a daemon?

Clipboard history requires continuous monitoring.

Keeping this functionality inside a background daemon avoids repeatedly initializing clipboard access, loading the database, and starting the UI process.

The UI becomes a temporary client that connects only when needed, allowing fast startup while keeping background resource usage low.

---

# Current Status

> **Early development**
>
> Clipstash is functional but still under active development. Features, APIs, and storage formats may change without notice.

## Currently implemented

* Clipboard monitoring
* SQLite-backed clipboard history
* Full-text search
* Fast popup UI
* Live daemon → UI communication
* Selecting an entry restores it to the system clipboard
* Automatic reconnection between UI and daemon

## Currently in progress

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
````

Start the daemon:

```bash
cargo run -p clipd --release
```

Launch the UI manually:

```bash
cargo run -p clipui --release
```

Or use the launcher:

```bash
cargo run -p clip-launcher --release
```

The launcher requests the daemon to display the UI:

```text
    clip-launcher
          |
          | RequestUi
          v
        clipd
          |
          v
       clipui
```

The UI can be started before or after the daemon. If the daemon is unavailable, the UI will automatically reconnect once it becomes available.

---

# Architecture

```text
+-------------------+      RequestUi      +----------------------+
|  clip-launcher    | ------------------> |        clipd         |
+-------------------+                     |----------------------|
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
                                          |    egui popup UI     |
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

## clip-launcher

The launcher is responsible for:

* Requesting visibility of the UI

---

# Storage

Clipboard history is stored in a SQLite database.

Current and planned storage modes include:

* Temporary on-disk database (currently used for development and planned for removal)
* In-memory database (planned)
* Persistent database location (planned)

The planned in-memory mode provides an ephemeral clipboard history that disappears after shutdown, making it useful for users who prefer not to leave clipboard data on disk.

---

# Design Goals

Clipstash aims to be:

* Fast
* Lightweight
* Responsive
* Minimalistic
* Native to Linux
* Easy to maintain
* Fully written in Rust

Performance is a primary design goal.

The daemon remains running while the popup UI only connects when needed. This keeps startup latency low while avoiding unnecessary background resource usage.

Approximate idle resource usage:

* **clipd:** ~8 MB RAM, negligible CPU usage
* **clipui:** ~120 MB RAM, negligible CPU usage (typically below 1% on an AMD Ryzen 5800X)
* **clip-launcher:** negligible resource usage

Local socket communication is typically sub-millisecond; UI rendering is currently the dominant latency factor.

---

# Roadmap

## Privacy

* [ ] Restrict clipboard-related logging to trace mode only
* [ ] Add configuration warnings when enabling verbose logging, as logs may contain clipboard contents

## User Experience

* [x] Popup UI
* [x] Live daemon → UI updates
* [x] Clipboard restoration
* [ ] Paste directly into the previously focused application (only possible on X11)
* [ ] Better keyboard navigation (arrow keys, enter key selection, etc.)
* [ ] Image clipboard support
* [ ] Thumbnail generation pipeline

## Performance

* [x] SQLite Full-Text Search
* [ ] In-memory SQLite storage
* [ ] Startup and rendering benchmarks
* [ ] Additional performance optimizations

## Reliability

* [x] Automatic socket reconnection
* [ ] Automatically start UI when requested by the launcher
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

Clipstash is currently maintained as a personal learning project.

Bug reports, feature suggestions, and discussions are welcome.

At this stage, external code contributions may be limited while the architecture is still evolving.
