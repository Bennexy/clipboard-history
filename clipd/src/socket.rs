use std::env;
use std::fs;
use std::os::unix::net::SocketAddr;
use std::path::PathBuf;

use clip_common::connection::Connection;
use clip_common::get_socket_path;
use clip_common::messages::ServerMessage;
use clip_common::messages::{ClientRequest, ServerResponse};
use clip_common::model::ClipboardEntry;
use serde::Deserialize;
use serde::Serialize;
use serde_json;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::UnixListener;
use tokio::net::UnixStream;
use tokio::sync::broadcast;
use tokio::sync::mpsc::Sender;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;

use crate::db::DbRequest;

// todo: in the future also push new clips directly to the client.
// todo: i need to make this loop more robust. i want to have a loop for handling socket failures and a inner loop for handling message exchanges.
async fn handle_client(
    stream: UnixStream,
    tx: Sender<DbRequest>,
    event_tx: Sender<(ClipboardEntry, tokio::sync::oneshot::Sender<Result<(), arboard::Error>>)>,
) -> anyhow::Result<()> {
    let (reader, writer) = stream.into_split();
    let mut connection = Connection::new(reader, writer);

    loop {
        let request: ClientRequest = match connection.receive().await {
            Ok(Some(msg)) => msg,
            Ok(None) => {
                tracing::info!("Client disconnected");
                break;
            }
            Err(err) => {
                tracing::warn!("Invalid client request: {err}");
                break;
            }
        };

        trace!("Received a message {:?}", request);

        match request {
            ClientRequest::GetClipboardHistory { limit } => {
                tracing::trace!("Handling GetAllClipboard");
                let (response_tx, response_rx) = tokio::sync::oneshot::channel();
                tx.send(DbRequest::GetAllClipboard { limit, response: response_tx }).await?;
                let entries = response_rx.await?;
                connection.send::<ServerMessage>(&ServerResponse::ClipboardEntries(entries).into()).await?
            }

            ClientRequest::SearchClipboardHistory { limit, query } => {
                tracing::trace!("Handling SearchTextClipboard");
                let (response_tx, response_rx) = tokio::sync::oneshot::channel();
                tx.send(DbRequest::SearchTextClipboard { limit, search_string: query, response: response_tx }).await?;
                let entries = response_rx.await?;
                connection.send::<ServerMessage>(&ServerResponse::ClipboardEntries(entries).into()).await?;
            }

            ClientRequest::SetClipboardEntry { id } => {
                let response = set_clipboard_entry(id, &tx, &event_tx).await?;
                connection.send::<ServerMessage>(&response.into()).await?;
            }
        }
    }

    Ok(())
}

pub async fn run(
    mut shutdown: broadcast::Receiver<()>,
    tx: Sender<DbRequest>,
    event_tx: Sender<(ClipboardEntry, tokio::sync::oneshot::Sender<Result<(), arboard::Error>>)>,
) -> anyhow::Result<()> {
    let socket = get_socket_path();

    // Remove old socket if it exists
    let _ = fs::remove_file(&socket);

    let listener = UnixListener::bind(&socket)
        .unwrap_or_else(|_| panic!("Failed to bind to the socket: {}", socket.to_string_lossy()));

    info!("Socket listening on {:?}", socket);

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                tracing::info!("Socket server shutting down");
                break;
            }

            result = listener.accept() => {
                let (stream, _) = match result {
                    Ok(conn) => conn,
                    Err(err) => {
                        error!("Failed to accept Unix stream: {err}");
                        continue;
                    }
                };
                debug!("Client connected");

                tokio::spawn(handle_client(stream, tx.clone(), event_tx.clone()));
            }
        }
    }

    let _ = std::fs::remove_file(get_socket_path());

    Ok(())
}

async fn set_clipboard_entry(
    id: i64,
    tx: &Sender<DbRequest>,
    event_tx: &Sender<(ClipboardEntry, tokio::sync::oneshot::Sender<Result<(), arboard::Error>>)>,
) -> anyhow::Result<ServerResponse> {
    tracing::trace!("Handling SetClipboardEntry");

    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    tx.send(DbRequest::GetById { id, response: response_tx }).await?;

    let entry = match response_rx.await? {
        Some(entry) => entry,
        None => {
            error!(
                "Failed to set the clipboard to the requested entry due since the entry no longer exsists in the db."
            );
            return Ok(ServerResponse::Error("Clipboard entry no longer exsists.".into()));
        }
    };

    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    event_tx.send((entry, response_tx)).await?;
    match response_rx.await? {
        Ok(()) => Ok(ServerResponse::Success),
        Err(error) => {
            error!("Failed to set the clipboard to the requested entry due to {:?}", error);
            Ok(ServerResponse::Error(error.to_string()))
        }
    }
}
