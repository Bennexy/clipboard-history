use serde::{Deserialize, Serialize};

use crate::model::ClipboardEntry;

/**
 * Client -> Server
 */
#[derive(Serialize, Deserialize, Debug)]
pub enum ClientRequest {
    GetClipboardHistory { limit: usize },
    SearchClipboardHistory { limit: usize, query: String },
}

/**
 * Server -> Client
 */
#[derive(Serialize, Deserialize, Debug)]
pub enum ServerResponse {
    ClipboardEntries(Vec<ClipboardEntry>),
}
