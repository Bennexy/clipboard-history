use anyhow::Error;
use tokio::sync::mpsc::{Receiver, Sender};

use clip_common::connection::Connection;
use clip_common::get_socket_path;
use clip_common::messages::{ClientRequest, ServerResponse};

// todo: i need to make this loop more robust. i want to have a loop for handling socket failures and a inner loop for handling message exchanges.
pub async fn run(mut commands: Receiver<ClientRequest>, responses: Sender<ServerResponse>) -> anyhow::Result<()> {
    let stream = tokio::net::UnixStream::connect(get_socket_path()).await?;
    let (reader, writer) = stream.into_split();
    let mut connection = Connection::new(reader, writer);

    loop {
        let Some(request) = commands.recv().await else {
            continue;
        };

        match connection.send(&request).await {
            Ok(_) => (),
            Err(err) => {
                tracing::error!("Couldn't send message to the daemon due to {err}");
                continue;
            }
        };

        match connection.receive().await {
            Ok(Some(msg)) => match responses.send(msg).await {
                Ok(_) => (),
                Err(err) => {
                    tracing::error!("Couldn't send server reponse to the UI due to {err}");
                    continue;
                }
            },
            Ok(None) => {
                tracing::error!("Daemon disconnected!");
                return Err(Error::msg("Daemon is disconnected. Is the service down?"));
            }
            Err(err) => {
                tracing::warn!("Invalid server response: {err}");
                continue;
            }
        };
    }
}
