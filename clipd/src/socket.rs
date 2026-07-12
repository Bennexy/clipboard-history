use std::env;
use std::fs;
use std::os::unix::net::SocketAddr;
use std::path::PathBuf;

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

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum SocketMessage {
    GetAllClipboard { request_id: usize, limit: usize },

    SearchTextClipboard { request_id: usize, limit: usize, query: String },
}

#[derive(Debug, Serialize)]
pub struct SocketResponse<T> {
    pub request_id: usize,
    pub data: T,
}

// todo: in the future also push new clips directly to the client.
async fn handle_client(mut stream: UnixStream, tx: Sender<DbRequest>) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.split();

    let mut reader = BufReader::new(reader);
    let mut command = Vec::new();

    loop {
        command.clear();

        let bytes_read = reader.read_until(0, &mut command).await?;
        let _ = command.pop_if(|x| *x == 0);

        trace!("Recieved a message {}", String::from_utf8_lossy(&command));

        // Client disconnected
        if bytes_read == 0 {
            tracing::debug!("Client disconnected");
            break;
        }

        let message: SocketMessage = match serde_json::from_slice(&command) {
            Ok(msg) => msg,
            Err(err) => {
                tracing::warn!("Invalid socket message: {err}");
                continue;
            }
        };

        match message {
            SocketMessage::GetAllClipboard { request_id, limit } => {
                tracing::trace!("Handling GetAllClipboard");

                let (response_tx, response_rx) = tokio::sync::oneshot::channel();

                tx.send(DbRequest::GetAllClipboard { limit, response: response_tx }).await?;

                let entries = response_rx.await?;

                let response = SocketResponse { request_id, data: entries };

                // trace!("Writing data: to socket! {:#?}", response);
                let mut bytes = serde_json::to_vec(&response)?;
                bytes.push(0);

                writer.write_all(&bytes).await?;
            }

            SocketMessage::SearchTextClipboard { request_id, limit, query } => {
                tracing::trace!("Handling SearchTextClipboard");

                let (response_tx, response_rx) = tokio::sync::oneshot::channel();

                tx.send(DbRequest::SearchTextClipboard { limit, search_string: query, response: response_tx }).await?;

                let entries = response_rx.await?;

                let response = SocketResponse { request_id, data: entries };

                let mut bytes = serde_json::to_vec(&response)?;
                bytes.push(0);

                writer.write_all(&bytes).await?;
            }
        }
    }

    Ok(())
}

pub async fn run(mut shutdown: broadcast::Receiver<()>, tx: Sender<DbRequest>) -> anyhow::Result<()> {
    let socket = socket_path();

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

    let _ = std::fs::remove_file(socket_path());

    Ok(())
}

fn socket_path() -> PathBuf {
    let runtime_dir = env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR is not set");

    PathBuf::from(runtime_dir).join("clipstash.sock")
}
