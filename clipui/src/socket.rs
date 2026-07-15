use std::{env, path::PathBuf};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::trace;

use clip_common::get_socket_path;
use clip_common::model::ClipboardEntry;

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ClientCommand {
    GetAllClipboard { request_id: usize, limit: usize },

    SearchTextClipboard { request_id: usize, limit: usize, query: String },
}

#[derive(Debug, Deserialize)]
pub struct ClientResponse {
    pub request_id: usize,
    pub data: Vec<ClipboardEntry>,
}

pub async fn run(mut commands: Receiver<ClientCommand>, responses: Sender<ClientResponse>) -> anyhow::Result<()> {
    let mut stream = tokio::net::UnixStream::connect(get_socket_path()).await?;

    let (reader, mut writer) = stream.split();

    let mut reader = BufReader::new(reader);

    loop {
        if let Some(command) = commands.recv().await {
            trace!("Recieved a client command! Will process now.");
            let mut request = serde_json::to_vec(&command)?;
            request.push(0);

            trace!("Writing the request to the socket: {}", String::from_utf8_lossy(&request));
            writer.write_all(&request).await?;
            trace!("Wrote request to the socket!");

            let mut response = Vec::new();

            reader.read_until(0, &mut response).await?;
            let _ = response.pop_if(|x| *x == 0);
            // trace!("recieved response from server. {}", String::from_utf8_lossy(&response));

            let response: ClientResponse = serde_json::from_slice(&response).expect("Failed to parse the response!");

            responses.send(response).await?;
            trace!("Successfully process the client command.");
        }
    }
}
