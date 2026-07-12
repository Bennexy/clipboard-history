## Project Summary: Rust Linux Clipboard History Manager

A privacy-focused, ultra-fast Linux clipboard manager written in Rust.

The application consists of two main parts:

```text
                    ┌──────────────────┐
                    │   User presses   │
                    │   Ctrl+C         │
                    └────────┬─────────┘
                             |
                             v
                    ┌──────────────────┐
                    │ clipboard daemon │
                    │     clipd        │
                    └────────┬─────────┘
                             |
              ┌──────────────┴──────────────┐
              v                             v
     RAM-backed session data          Persistent storage
     (fast access)                    (large payloads)

     - SQLite FTS index               - Original images/files
     - Metadata                       - Large clipboard blobs
     - Thumbnail cache

                             |
                             v

                    ┌──────────────────┐
                    │   clipui         │
                    │   egui popup     │
                    └──────────────────┘

                    Search → Preview → Select → Paste
```

### Core goals

* Capture clipboard history automatically.
* Support text and images.
* Provide instant search-as-you-type.
* Provide image previews.
* Keep history temporary by default.
* Be extremely fast:

  * instant popup
  * no long loading
  * no UI lag
* Use Rust end-to-end.
* Integrate naturally with Linux.

---

# Implementation Roadmap

## Phase 1 — Project foundation

### Goal

Create the Rust workspace and basic application structure.

### Tools

* Rust
* Cargo workspace
* Git

Recommended structure:

```text
clipstash/
├── clipd/
├── clipui/
└── shared/
```

### Completion criteria

✅ Workspace builds
✅ Separate daemon and UI binaries exist
✅ Shared data structures compile between projects

---

# Phase 2 — Clipboard daemon: text only

## Goal

Create a background process that detects clipboard changes and stores text.

### Tools

Rust crates:

* `arboard` (initial clipboard abstraction)
* later:

  * `x11rb`
  * Wayland libraries

Storage:

* SQLite

Crates:

```toml
rusqlite
serde
chrono
```

---

### Implementation

Daemon loop:

```text
clipboard changed?
        |
        v
read text
        |
        v
store entry
```

Database:

```sql
entries

id
timestamp
text
type
```

---

### Completion criteria

✅ Daemon runs independently
✅ Copying text creates a history entry
✅ Multiple copies are stored
✅ Duplicate handling exists

---

# Phase 3 — Temporary session storage

## Goal

Make clipboard history disappear after reboot.

### Tools

Linux:

* `/run/user/$UID`
* tmpfs

SQLite:

* WAL mode
* memory cache

---

Architecture:

```text
/run/user/1000/clipstash/

history.db
```

---

### Completion criteria

✅ Database exists only during session
✅ Reboot clears history automatically
✅ No manual cleanup required

---

# Phase 4 — Full text search

## Goal

Implement instant search.

### Tools

SQLite:

* FTS5

Example:

```sql
CREATE VIRTUAL TABLE entries_fts
USING fts5(text);
```

---

Search flow:

```text
User types:

dock

wait 300ms

SQLite FTS query

return results
```

---

### Completion criteria

✅ Search works while typing
✅ Results appear within milliseconds
✅ Thousands of entries remain instant

---

# Phase 5 — egui user interface

## Goal

Create the clipboard picker.

### Tools

Rust:

* `egui`
* `eframe`

UI:

```
+--------------------------+
| Search                   |
+--------------------------+
| docker compose up        |
| ssh server               |
| cargo build              |
|                          |
| preview                  |
+--------------------------+
```

---

Features:

* search field
* result list
* keyboard navigation
* selection

---

### Completion criteria

✅ UI opens quickly
✅ Results display
✅ Arrow keys navigate
✅ Enter selects an item

---

# Phase 6 — Clipboard replacement

## Goal

Paste selected history entry.

Flow:

```text
User selects entry

        |
        v

clipui tells clipd

        |
        v

clipd sets clipboard

        |
        v

user pastes normally
```

---

Tools:

* clipboard crates
* Unix socket communication

---

### Completion criteria

✅ Selecting an entry replaces clipboard
✅ Normal Ctrl+V works afterward

---

# Phase 7 — Image clipboard support

## Goal

Store and preview images.

### Tools

Rust:

* `image`
* `webp`
* filesystem APIs

---

Storage:

```
Persistent:

images/
    original.png


RAM:

thumbnail cache
```

---

Pipeline:

```text
Clipboard image

        |
        v

Save original

        |
        v

Generate thumbnail

        |
        v

RAM cache

        |
        v

egui preview
```

---

### Completion criteria

✅ Images appear in history
✅ Original files are not loaded into RAM
✅ Preview appears instantly

---

# Phase 8 — Thumbnail cache

## Goal

Keep UI extremely fast.

### Tools

Rust:

* `lru`

Design:

```text
RAM:

thumbnail cache

maximum:
256 MB
```

---

Example:

```text
500 images

each:
50-100 KB

total:
25-50 MB
```

---

### Completion criteria

✅ Opening UI does not decode all images
✅ Old thumbnails are automatically removed
✅ Memory limit is respected

---

# Phase 9 — IPC between daemon and UI

## Goal

Separate UI and backend.

### Tools

Linux:

* Unix domain sockets

Rust:

* `tokio`
* `serde_json`

Architecture:

```
clipui

   |
   | search request
   |
   v

clipd

   |
   | database query
   |
   v

results
```

---

### Completion criteria

✅ UI does not access database directly
✅ Daemon can run without UI
✅ Multiple UI requests work

---

# Phase 10 — Global hotkey

## Goal

Open the picker instantly.

Example:

```
Ctrl + Shift + V
```

### Tools

Rust:

* `global-hotkey`

Flow:

```
shortcut pressed

        |

start clipui

        |

focus search
```

---

### Completion criteria

✅ Hotkey works globally
✅ Popup appears immediately
✅ Keyboard workflow works without mouse

---

# Phase 11 — Linux integration improvements

## Goal

Move from "works everywhere" to "native".

### Tools

X11:

* `x11rb`

Wayland:

* Wayland protocols

---

Improvements:

* event-based clipboard monitoring
* better image MIME handling
* native clipboard ownership

---

### Completion criteria

✅ Works on X11
✅ Works on Wayland
✅ No polling dependency where possible

---

# Phase 12 — Polish and production readiness

## Add:

### Limits

```text
Maximum entries
Maximum disk usage
Maximum image size
Maximum RAM cache
```

---

### Privacy features

* temporary mode (default)
* optional persistence
* clear history command
* per-entry deletion

---

### Quality

* error handling
* logging
* systemd user service
* packaging

---

# Final Technology Stack

| Component  | Choice                                |
| ---------- | ------------------------------------- |
| Language   | Rust                                  |
| UI         | egui + eframe                         |
| Database   | SQLite                                |
| Search     | SQLite FTS5                           |
| Async      | Tokio                                 |
| IPC        | Unix domain sockets                   |
| Clipboard  | arboard → native X11/Wayland later    |
| Images     | image crate                           |
| Thumbnails | WebP                                  |
| Cache      | LRU                                   |
| Storage    | `/run/user/$UID` + secure disk folder |
| Hotkeys    | global-hotkey                         |

---

## Recommended first milestone

Do **not** start with images or Wayland.

The smallest useful version is:

```
Ctrl+C
    |
    v
Rust daemon
    |
    v
SQLite
    |
    v
egui window
    |
    v
search + select
```

Once that works, everything else becomes an extension rather than a rewrite.
