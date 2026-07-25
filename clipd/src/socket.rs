use anyhow::Result;
use clip_common::messages::ServerEvent;
use clip_common::messages::SocketMessage;
use std::fs;
use tokio::net::unix::OwnedReadHalf;
use tokio::net::unix::OwnedWriteHalf;

use clip_common::connection::Connection;
use clip_common::get_socket_path;
use clip_common::messages::ServerMessage;
use clip_common::messages::{ClientRequest, ServerResponse};
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
    event_tx: Sender<(i64, tokio::sync::oneshot::Sender<Result<ServerResponse>>)>,
    mut event_rx: broadcast::Receiver<ServerEvent>,
) -> Result<()> {
    let (reader, writer) = stream.into_split();
    let mut connection = Connection::new(reader, writer);

    loop {
        tokio::select! {
            request = connection.receive() => {
                match request {
                    Ok(Some(msg)) => handle_message(msg, &mut connection, &tx, &event_tx).await?,
                    Ok(None) => {
                        tracing::info!("Client disconnected");
                        break;
                    }
                    Err(err) => {
                        tracing::warn!("Invalid client request: {err}");
                        break;
                    }
                };
            }

            event = event_rx.recv() => {
                handle_event(event?, &mut connection).await?;
            }
        }
    }

    Ok(())
}

async fn handle_event(event: ServerEvent, connection: &mut Connection<OwnedReadHalf, OwnedWriteHalf>) -> Result<()> {
    connection.send(&SocketMessage::ServerMessage(ServerMessage::Event(event))).await
}

async fn handle_message(
    request: SocketMessage,
    connection: &mut Connection<OwnedReadHalf, OwnedWriteHalf>,
    tx: &Sender<DbRequest>,
    event_tx: &Sender<(i64, tokio::sync::oneshot::Sender<Result<ServerResponse>>)>,
) -> Result<()> {
    trace!("Received a message {:?}", request);

    match request {
        SocketMessage::GlobalEvent(_) | SocketMessage::ServerMessage(_) => (),
        SocketMessage::ClientMessage(client_request) => match client_request {
            ClientRequest::GetClipboardHistory { limit } => {
                tracing::trace!("Handling GetAllClipboard");
                let (response_tx, response_rx) = tokio::sync::oneshot::channel();
                tx.send(DbRequest::GetAllClipboard { limit, response: response_tx }).await?;
                let entries = response_rx.await?;
                connection
                    .send(&SocketMessage::ServerMessage(ServerMessage::Response(ServerResponse::ClipboardEntries(
                        entries,
                    ))))
                    .await?;
            }

            ClientRequest::SearchClipboardHistory { limit, query } => {
                tracing::trace!("Handling SearchTextClipboard");
                let (response_tx, response_rx) = tokio::sync::oneshot::channel();
                tx.send(DbRequest::SearchTextClipboard { limit, search_string: query, response: response_tx }).await?;
                let entries = response_rx.await?;
                connection
                    .send(&SocketMessage::ServerMessage(ServerMessage::Response(ServerResponse::ClipboardEntries(
                        entries,
                    ))))
                    .await?;
            }

            ClientRequest::SetClipboardEntry { id } => {
                tracing::trace!("Handling SetClipboardEntry");
                let (response_tx, response_rx) = tokio::sync::oneshot::channel::<Result<ServerResponse>>();

                event_tx.send((id, response_tx)).await?;
                let res = match response_rx.await? {
                    Ok(response) => response,
                    Err(error) => {
                        error!("Failed to set the clipboard to the requested entry due to {:?}", error);
                        ServerResponse::Error(error.to_string())
                    }
                };
                tracing::info!("Sending: {:#?}", res);
                connection.send(&SocketMessage::ServerMessage(ServerMessage::Response(res))).await?;
            }
        },
    }

    Ok(())
}

pub async fn run(
    mut shutdown: broadcast::Receiver<()>,
    tx: Sender<DbRequest>,
    event_tx: Sender<(i64, tokio::sync::oneshot::Sender<Result<ServerResponse>>)>,
    event_rx: broadcast::Receiver<ServerEvent>,
) -> Result<()> {
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

                tokio::spawn(handle_client(stream, tx.clone(), event_tx.clone(), event_rx.resubscribe()));
            }
        }
    }

    let _ = std::fs::remove_file(get_socket_path());

    Ok(())
}
