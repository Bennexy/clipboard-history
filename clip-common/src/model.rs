#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ClipboardEntry {
    pub id: i64,
    pub mime_type: String,
    pub created_at: i64,
    pub text: String,
}
