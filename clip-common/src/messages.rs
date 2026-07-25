use serde::{Deserialize, Serialize};

use crate::model::ClipboardEntry;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SocketMessage {
    ClientMessage(ClientRequest),
    ServerMessage(ServerMessage),
    GlobalEvent(GlobalEvent),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GlobalEvent {
    ShowUi,
}

/**
 * Client -> Server
 */
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClientRequest {
    GetClipboardHistory { limit: usize },
    SearchClipboardHistory { limit: usize, query: String },
    SetClipboardEntry { id: i64 },
}

/**
 * Server -> Client
 */

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerMessage {
    Response(ServerResponse),
    Event(ServerEvent),
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerResponse {
    ClipboardEntries(Vec<ClipboardEntry>),
    Success,
    Error(String),
}

impl From<ServerResponse> for ServerMessage {
    fn from(value: ServerResponse) -> Self {
        ServerMessage::Response(value)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerEvent {
    NewClipboardEntry(ClipboardEntry),
}

impl From<ServerEvent> for ServerMessage {
    fn from(value: ServerEvent) -> Self {
        ServerMessage::Event(value)
    }
}
