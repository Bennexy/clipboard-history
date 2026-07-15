use std::env;
use std::fs;
use std::os::unix::net::SocketAddr;
use std::path::PathBuf;

use clip_common::connection::Connection;
use clip_common::get_socket_path;
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
async fn handle_client(stream: UnixStream, tx: Sender<DbRequest>) -> anyhow::Result<()> {
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
                connection.send(&ServerResponse::ClipboardEntries(entries)).await?
            }

            ClientRequest::SearchClipboardHistory { limit, query } => {
                tracing::trace!("Handling SearchTextClipboard");
                let (response_tx, response_rx) = tokio::sync::oneshot::channel();
                tx.send(DbRequest::SearchTextClipboard { limit, search_string: query, response: response_tx }).await?;
                let entries = response_rx.await?;
                connection.send(&ServerResponse::ClipboardEntries(entries)).await?;
            }
        }
    }

    Ok(())
}

pub async fn run(mut shutdown: broadcast::Receiver<()>, tx: Sender<DbRequest>) -> anyhow::Result<()> {
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

                tokio::spawn(handle_client(stream, tx.clone()));
            }
        }
    }

    let _ = std::fs::remove_file(get_socket_path());

    Ok(())
}
