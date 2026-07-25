use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};
use tokio::sync::{broadcast, mpsc::Receiver};
use tracing::{debug, trace};

use crate::clipboard::ClipboardEvent;
use clip_common::model::ClipboardEntry;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Event {
    NewClipboardEntry(ClipboardEntry),
}

pub enum DbRequest {
    GetAllClipboard {
        limit: usize,
        response: tokio::sync::oneshot::Sender<Vec<ClipboardEntry>>,
    },
    SearchTextClipboard {
        limit: usize,
        search_string: String,
        response: tokio::sync::oneshot::Sender<Vec<ClipboardEntry>>,
    },
    GetById {
        id: i64,
        response: tokio::sync::oneshot::Sender<Option<ClipboardEntry>>,
    },
    CreateClipboardEntry(ClipboardEvent),
}

// need to think about the thumbnail thingy...
// TODO: write the sqlite file into memory as make ephirial!
fn setup_db() -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open("/home/ben/dev/clipboard-history/clipd.db")?;

    trace!("Database at '/home/ben/dev/clipboard-history/clipd.db' opened!");
    conn.execute_batch(
        "
    PRAGMA foreign_keys = ON;
    PRAGMA journal_mode=WAL;
    PRAGMA synchronous=OFF;
    PRAGMA busy_timeout = 1000;
    PRAGMA cache_size = -65536;
    PRAGMA temp_store = MEMORY;

    CREATE TABLE IF NOT EXISTS clipboard_entries (
        id INTEGER PRIMARY KEY,
        mime_type TEXT NOT NULL,
        size INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_clipboard_entries_created_at
        ON clipboard_entries(created_at DESC);


    CREATE TABLE IF NOT EXISTS clipboard_text (
        entry_id INTEGER PRIMARY KEY,
        text TEXT NOT NULL,

        FOREIGN KEY(entry_id)
            REFERENCES clipboard_entries(id)
            ON DELETE CASCADE
    );


    CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_fts
    USING fts5(
        text,
        content='clipboard_text',
        content_rowid='entry_id'
    );

    CREATE TRIGGER IF NOT EXISTS clipboard_text_insert
    AFTER INSERT ON clipboard_text
    BEGIN
        INSERT INTO clipboard_fts(rowid, text)
        VALUES (new.entry_id, new.text);
    END;

    CREATE TRIGGER IF NOT EXISTS clipboard_text_delete
    AFTER DELETE ON clipboard_text
    BEGIN
        INSERT INTO clipboard_fts(
            clipboard_fts,
            rowid,
            text
        )
        VALUES(
            'delete',
            old.entry_id,
            old.text
        );
    END;


    CREATE TABLE IF NOT EXISTS clipboard_images (
        entry_id INTEGER PRIMARY KEY,

        path TEXT NOT NULL,
        thumbnail_path TEXT,
        thumbnail_width INTEGER,
        thumbnail_height INTEGER,

        FOREIGN KEY(entry_id)
            REFERENCES clipboard_entries(id)
            ON DELETE CASCADE
    );
    ",
    )?;

    Ok(conn)
}

pub async fn run(
    mut shutdown: broadcast::Receiver<()>,
    mut event_rx: Receiver<DbRequest>,
    mut request_rx: Receiver<DbRequest>,
    event_tx: broadcast::Sender<Event>,
) -> anyhow::Result<()> {
    let mut conn = setup_db().expect("Failed to setup the sqlite.db!");
    debug!("Successfully initialized the db!");

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                tracing::info!("DB worker shutting down");
                break;
            }
            Some(request) = event_rx.recv() => {
                request_recieve::handle_request_recieve(&mut conn, request)?;
            },
            Some(request) = request_rx.recv() => {
                let _ = request_recieve::handle_request_recieve(&mut conn, request);
            }
        }
    }

    Ok(())
}

mod event_recieve {
    use super::*;
    pub fn handle_event_recieve(conn: &mut Connection, event: ClipboardEvent) -> ClipboardEntry {
        match event {
            ClipboardEvent::Text(text) => {
                debug!("Handling a text clipboard change in sqlite code");
                let entry = create_text_entry(conn, text).expect("Failed to store the text data!");
                debug!("Finished handling the text clipboard change.");
                entry
            }
            ClipboardEvent::Image(image) => {
                unimplemented!("image handling not yet implemented!");
            }
        }
    }

    fn create_text_entry(conn: &mut Connection, text: String) -> Result<ClipboardEntry, rusqlite::Error> {
        let tx = conn.transaction()?;

        let created_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

        let size = text.len() as i64;
        let mime_type = "text/plain".to_string();

        // Insert main clipboard entry
        tx.execute(
            "
        INSERT INTO clipboard_entries (
            mime_type,
            size,
            created_at
        )
        VALUES (?1, ?2, ?3)
        ",
            params![mime_type, size, created_at,],
        )?;

        let id = tx.last_insert_rowid();

        // Insert text content
        tx.execute(
            "
        INSERT INTO clipboard_text (
            entry_id,
            text
        )
        VALUES (?1, ?2)
        ",
            params![id, &text],
        )?;

        tx.commit()?;

        Ok(ClipboardEntry { id, mime_type, created_at, text })
    }
}

mod request_recieve {
    use std::time::Instant;

    use rusqlite::OptionalExtension;

    use super::*;

    pub fn handle_request_recieve(conn: &mut Connection, request: DbRequest) -> anyhow::Result<()> {
        let now = Instant::now();

        match request {
            DbRequest::GetAllClipboard { limit, response } => {
                trace!("Recieved a DbRequest::GetAllClipboard request which will be served now.");
                response.send(get_all_clipboard(conn, limit)).expect("Failed to send the db response to the reciever.");
                trace!("Served the DbRequest::GetAllClipboard request and responded.");
            }
            DbRequest::SearchTextClipboard { limit, search_string, response } => {
                trace!("Recieved a DbRequest::SearchTextClipboard request which will be served now.");

                let results;
                if let Some(query) = build_fts_query(search_string) {
                    results = search_text(conn, query, limit);
                } else {
                    results = get_all_clipboard(conn, limit);
                }
                response.send(results).expect("Failed to send the db response to the reciever.");
                trace!("Served the DbRequest::SearchTextClipboard request and responded.");
            }
            DbRequest::GetById { id, response } => {
                trace!("Recieved a DbRequest::GetById request which will be served now.");
                response.send(get_by_id(conn, id)).expect("Failed to send the db response to the reciever.");
            }
            DbRequest::CreateClipboardEntry(event) => {
                event_recieve::handle_event_recieve(conn, event);
            }
        };
        tracing::error!(
            "Db request fully processed after {}ms -> {}ns",
            now.elapsed().as_millis(),
            now.elapsed().as_nanos()
        );

        Ok(())
    }

    fn build_fts_query(input: String) -> Option<String> {
        let mut query = String::new();

        for word in input.split_whitespace() {
            let cleaned: String = word.chars().filter(|c| c.is_alphanumeric()).collect();

            if cleaned.is_empty() {
                continue;
            }

            if !query.is_empty() {
                query.push(' ');
            }

            query.push_str(&cleaned);
            query.push('*');
        }

        (!query.is_empty()).then_some(query)
    }

    fn get_all_clipboard(conn: &Connection, limit: usize) -> Vec<ClipboardEntry> {
        let mut stmt = conn
            .prepare(
                "
                SELECT
                    e.id,
                    e.mime_type,
                    e.created_at,
                    t.text
                FROM clipboard_entries e
                LEFT JOIN clipboard_text t
                    ON e.id = t.entry_id
                ORDER BY e.created_at DESC
                LIMIT ?1",
            )
            .unwrap();

        stmt.query_map([limit as i64], |row| {
            Ok(ClipboardEntry { id: row.get(0)?, mime_type: row.get(1)?, created_at: row.get(2)?, text: row.get(3)? })
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    }

    fn search_text(conn: &Connection, search_string: String, limit: usize) -> Vec<ClipboardEntry> {
        let mut stmt = conn
            .prepare(
                "
                SELECT
                    e.id,
                    e.mime_type,
                    e.created_at,
                    t.text
                FROM clipboard_fts fts
                JOIN clipboard_text t
                    ON t.entry_id = fts.rowid
                JOIN clipboard_entries e
                    ON e.id = t.entry_id
                WHERE clipboard_fts MATCH ?1
                ORDER BY e.created_at DESC
                LIMIT ?2",
            )
            .unwrap();

        let rows = stmt
            .query_map([search_string, (limit as i64).to_string()], |row| {
                Ok(ClipboardEntry {
                    id: row.get(0)?,
                    mime_type: row.get(1)?,
                    created_at: row.get(2)?,
                    text: row.get(3)?,
                })
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        return rows;
    }

    // todo: maybe use a dedicated only text value for this?
    fn get_by_id(conn: &Connection, id: i64) -> Option<ClipboardEntry> {
        conn.query_one(
            "
            SELECT
                e.id,
                e.mime_type,
                e.created_at,
                t.text
            FROM clipboard_entries e
            LEFT JOIN clipboard_text t
                ON e.id = t.entry_id
            WHERE e.id = ?1",
            [id],
            |row| {
                Ok(Some(ClipboardEntry {
                    id: row.get(0)?,
                    mime_type: row.get(1)?,
                    created_at: row.get(2)?,
                    text: row.get(3)?,
                }))
            },
        )
        .unwrap()
    }
}
