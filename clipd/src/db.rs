use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};
use tokio::sync::{broadcast, mpsc::Receiver};
use tracing::{debug, trace};

use crate::clipboard::ClipboardEvent;

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
}

#[derive(Debug, serde::Serialize)]
pub struct ClipboardEntry {
    pub id: i64,
    pub mime_type: String,
    pub created_at: i64,
    pub text: Option<String>,
}

// need to think about the thumbnail thingy...
// TODO: write the sqlite file into memory as make ephirial!
fn setup_db() -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open("/home/ben/dev/clipboard-history/clipd.db")?;

    trace!("Database at '/home/ben/dev/clipboard-history/clipd.db' opened!");
    conn.execute_batch(
        "
    PRAGMA foreign_keys = ON;
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
    mut event_rx: Receiver<ClipboardEvent>,
    mut request_rx: Receiver<DbRequest>,
) -> anyhow::Result<()> {
    let mut conn = setup_db().expect("Failed to setup the sqlite.db!");
    debug!("Successfully initialized the db!");

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                tracing::info!("DB worker shutting down");
                break;
            }
            Some(event) = event_rx.recv() => {
                event_recieve::handle_event_recieve(&mut conn, event);
            },
            Some(request) = request_rx.recv() => {
                request_recieve::handle_request_recieve(&conn, request);
            }
        }
    }

    Ok(())
}

mod event_recieve {
    use super::*;
    pub fn handle_event_recieve(conn: &mut Connection, event: ClipboardEvent) {
        match event {
            ClipboardEvent::Text(text) => {
                debug!("Handling a text clipboard change in sqlite code");
                handle_text_event_recieve(conn, text).expect("Failed to store the text data!");
                debug!("Finished handling the text clipboard change.")
            }
            ClipboardEvent::Image(image) => {
                unimplemented!("image handling not yet implemented!");
            }
        };
    }

    fn handle_text_event_recieve(conn: &mut Connection, text: String) -> Result<(), rusqlite::Error> {
        let tx = conn.transaction()?;

        let created_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

        let size = text.len() as i64;

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
            params!["text/plain", size, created_at,],
        )?;

        let entry_id = tx.last_insert_rowid();

        // Insert text content
        tx.execute(
            "
        INSERT INTO clipboard_text (
            entry_id,
            text
        )
        VALUES (?1, ?2)
        ",
            params![entry_id, &text],
        )?;

        // Insert into FTS index
        tx.execute(
            "
        INSERT INTO clipboard_fts (
            rowid,
            text
        )
        VALUES (?1, ?2)
        ",
            params![entry_id, &text],
        )?;

        tx.commit()?;

        Ok(())
    }
}

mod request_recieve {
    use super::*;

    pub fn handle_request_recieve(conn: &Connection, request: DbRequest) {
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
        }
    }

    fn build_fts_query(input: String) -> Option<String> {
        let query = input
            .split_whitespace()
            .filter_map(|word| {
                let cleaned: String = word.chars().filter(|c| c.is_alphanumeric()).collect();

                if cleaned.is_empty() { None } else { Some(format!("{}*", cleaned)) }
            })
            .collect::<Vec<_>>()
            .join(" ");

        if query.is_empty() { None } else { Some(query) }
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

        let rows = stmt
            .query_map([limit as i64], |row| {
                Ok(ClipboardEntry {
                    id: row.get(0)?,
                    mime_type: row.get(1)?,
                    created_at: row.get(2)?,
                    text: row.get(3)?,
                })
            })
            .unwrap();

        rows.map(|r| r.unwrap()).collect()
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
            .unwrap();

        rows.map(|r| r.unwrap()).collect()
    }
}
