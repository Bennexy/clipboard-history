use std::{borrow::Cow, sync::Arc};

use ahash::AHasher;
use anyhow::{Error, Result};
use arboard::{Clipboard, ImageData};
use clip_common::{messages::ServerResponse, model::ClipboardEntry};
use std::hash::{Hash, Hasher};
use tokio::{
    sync::{
        broadcast,
        mpsc::{Receiver, Sender},
    },
    time::{Duration, sleep},
};
use tracing::{debug, error, info};

use crate::db::DbRequest;

#[derive(PartialEq)]
struct ImageFingerprint {
    width: usize,
    height: usize,
    size: usize,
    partial_hash: u64,
}

// todo: future improvement! (not really required)
#[derive(PartialEq)]
struct TextFingerprint {
    size: usize,
    hash: u64,
}

#[derive(PartialEq)]
enum ClipHash {
    Text(u64),
    Image(ImageFingerprint),
    Empty,
}

pub enum ClipboardEvent {
    Text(String),
    Image(Arc<[u8]>),
}

pub async fn run(
    mut shutdown: broadcast::Receiver<()>,
    tx: Sender<DbRequest>,
    mut rx: Receiver<(i64, tokio::sync::oneshot::Sender<Result<ServerResponse>>)>,
) -> anyhow::Result<()> {
    let duration = Duration::from_millis(500);
    let mut last_hash: ClipHash = ClipHash::Empty;
    let mut clipboard = Clipboard::new().expect("Failed to initialize clipboard");

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                info!("Clipboard worker shutting down gracefully");
                break;
            },
            _ = sleep(duration) => {
                poll_clipboard(&tx, &mut clipboard, &mut last_hash).await;
            }

            Some((event, response_tx)) = rx.recv() => {
                set_clipboard_entry(&mut clipboard, &tx, event, response_tx, &mut last_hash).await?;
            }
        }
    }

    Ok(())
}

async fn poll_clipboard(tx: &Sender<DbRequest>, clipboard: &mut Clipboard, last_hash: &mut ClipHash) {
    if let Ok(text) = clipboard.get_text() {
        let hash = ClipHash::Text(full_hash(&text));

        if *last_hash != hash {
            debug!("Clipboard contents changed - detected a text!");
            debug!("{}", text);

            *last_hash = hash;

            tx.send(DbRequest::CreateClipboardEntry(ClipboardEvent::Text(text)))
                .await
                .expect("Failed to send the ClipboardEvent::Text via the sender!");
        }
    } else if let Ok(image) = clipboard.get_image() {
        let is_changed = image_changed(last_hash, &image);

        if let Some(new_hash) = is_changed {
            debug!("Clipboard contents changed - detected a image!");
            *last_hash = new_hash;
            tx.send(DbRequest::CreateClipboardEntry(ClipboardEvent::Image(image.bytes.into())))
                .await
                .expect("Failed to send the ClipboardEvent::Image via the sender!");
        }
    }
}

async fn set_clipboard_entry(
    clipboard: &mut Clipboard,
    tx: &Sender<DbRequest>,
    entry_id: i64,
    response_tx: tokio::sync::oneshot::Sender<anyhow::Result<ServerResponse>>,
    last_hash: &mut ClipHash,
) -> Result<()> {
    let (db_response_tx, db_response_rx) = tokio::sync::oneshot::channel();
    tx.send(DbRequest::GetById { id: entry_id, response: db_response_tx }).await?;

    let entry: ClipboardEntry = match db_response_rx.await.unwrap() {
        Some(entry) => entry,
        None => {
            error!(
                "Failed to set the clipboard to the requested entry due since the entry no longer exsists in the db."
            );
            response_tx.send(Ok(ServerResponse::Error("Clipboard entry no longer exsists.".to_string()))).unwrap();
            return Ok(());
        }
    };

    let res = clipboard.set_text(&entry.text).map_err(anyhow::Error::new);
    *last_hash = ClipHash::Text(full_hash(&entry.text));
    debug!("set the clipboard entry to: {}", entry.text);
    response_tx.send(Ok(ServerResponse::Success)).unwrap();

    Ok(())
}

// returns None if the image did not change. Returns the new ClipHash in case it changed.
fn image_changed(last_hash: &ClipHash, current: &ImageData<'_>) -> Option<ClipHash> {
    match last_hash {
        ClipHash::Image(image) => {
            if !same_metadata(image, current) {
                return Some(ClipHash::Image(ImageFingerprint {
                    width: current.width,
                    height: current.height,
                    size: current.bytes.len(),
                    partial_hash: partial_hash(current.bytes.as_ref()),
                }));
            };

            let current_hash = partial_hash(current.bytes.as_ref());
            if current_hash == image.partial_hash {
                return None;
            }

            Some(ClipHash::Image(ImageFingerprint {
                width: current.width,
                height: current.height,
                size: current.bytes.len(),
                partial_hash: current_hash,
            }))
        }
        _ => None,
    }
}

fn same_metadata(state: &ImageFingerprint, image: &arboard::ImageData<'_>) -> bool {
    state.width == image.width && state.height == image.height && state.size == image.bytes.len()
}

fn partial_hash(bytes: &[u8]) -> u64 {
    let mut hasher = AHasher::default();

    let len = bytes.len();

    // Include size
    hasher.write(&len.to_le_bytes());

    if len == 0 {
        return hasher.finish();
    }

    const SAMPLE: usize = 4096;

    for pos in [0, len / 4, len / 2, (len * 3) / 4, len.saturating_sub(SAMPLE)] {
        let end = (pos + SAMPLE).min(len);
        hasher.write(&bytes[pos..end]);
    }

    hasher.finish()
}

fn full_hash(data: impl AsRef<[u8]>) -> u64 {
    let mut hasher = AHasher::default();
    hasher.write(data.as_ref());
    hasher.finish()
}
