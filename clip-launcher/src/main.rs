use tokio::net::UnixStream;

use clip_common::{
    connection::Connection,
    get_socket_path,
    messages::{GlobalEvent, SocketMessage},
};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let (reader, writer) = UnixStream::connect(get_socket_path()).await?.into_split();
    let mut connection = Connection::new(reader, writer);

    connection.send(&SocketMessage::GlobalEvent(GlobalEvent::ShowUi)).await?;
    info!("Successfully sent GlobalEvent::ShowUi via socket");

    Ok(())
}
