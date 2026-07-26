use anyhow::{Error, Result};
use tokio::net::UnixStream;
use tokio::sync::broadcast;
use tokio::sync::mpsc::{Receiver, Sender};

use clip_common::connection::Connection;
use clip_common::get_socket_path;
use clip_common::messages::{ClientRequest, SocketMessage};
use tokio::time::{Duration, sleep};
use tracing::{error, info, trace, warn};

// todo: i need to make this loop more robust. i want to have a loop for handling socket failures and a inner loop for handling message exchanges.
pub async fn run(
    mut shutdown_rx: broadcast::Receiver<()>,
    mut commands: Receiver<ClientRequest>,
    responses: Sender<SocketMessage>,
) -> Result<()> {
    let duration = Duration::from_millis(1500);
    loop {
        let stream = match tokio::net::UnixStream::connect(get_socket_path()).await {
            Ok(stream) => stream,
            Err(err) => {
                tracing::warn!(
                    "Failed to connect to the unix socket due to: {}. Is the Daemon running? Will retry in {:.2} seconds",
                    err,
                    duration.as_secs_f64()
                );
                sleep(duration).await;
                continue;
            }
        };

        trace!("Successfully initialized the socket connection!");
        match handle_messages(stream, &mut shutdown_rx, &mut commands, &responses).await {
            Ok(()) => {
                info!("Socket worker is sutting down gracefully");
                return Ok(());
            }
            Err(err) => {
                tracing::error!(
                    "Socket worker ran into an recoverable error: {}. Reconnecting to the socket.",
                    err.to_string()
                );
            }
        }
    }
}

async fn handle_messages(
    stream: UnixStream,
    shutdown_rx: &mut broadcast::Receiver<()>,
    commands: &mut Receiver<ClientRequest>,
    responses: &Sender<SocketMessage>,
) -> Result<()> {
    let (reader, writer) = stream.into_split();
    let mut connection: Connection<tokio::net::unix::OwnedReadHalf, tokio::net::unix::OwnedWriteHalf> =
        Connection::new(reader, writer);

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                return Ok(());
            },

            recieved_msg = connection.receive::<SocketMessage>() => {
                match recieved_msg {
                    Ok(Some(msg)) => match responses.send(msg.clone()).await {
                        Ok(_) => {
                            info!("socket worker recieved a message of type: {:?}", msg);},
                        Err(err) => {
                            error!("Couldn't send server reponse to the UI due to {err}");
                            continue;
                        }
                    },
                    Ok(None) => {
                        error!("Daemon disconnected!");
                        return Err(Error::msg("Daemon is disconnected. Is the service down?"));
                    }
                    Err(err) => {
                        warn!("Invalid server response: {err}");
                        continue;
                    }
                };
            },

            Some(request) = commands.recv() => {
                connection.send(&SocketMessage::ClientMessage(request)).await?;
            }
        }
    }

    // connection.close();
}
