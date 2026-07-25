use anyhow::{Result, bail};
use bytes::BytesMut;
use serde::{Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::error;

const HEADER_SIZE: usize = 4;
const MAX_FRAME_SIZE: usize = u32::MAX as usize; // 4Gib Max for now.

pub struct Connection<R, W> {
    reader: R,
    writer: W,
    // Reused for receiving
    read_buffer: BytesMut,

    // Reused for sending
    write_buffer: Vec<u8>,
}

impl<R, W> Connection<R, W>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub fn new(reader: R, writer: W) -> Self {
        Self { reader, writer, read_buffer: BytesMut::with_capacity(4096), write_buffer: Vec::with_capacity(4096) }
    }

    pub async fn send<T>(&mut self, message: &T) -> Result<()>
    where
        T: Serialize,
    {
        self.write_buffer.clear();
        self.write_buffer.extend_from_slice(&[0u8; HEADER_SIZE]);

        let buffer = std::mem::take(&mut self.write_buffer);
        self.write_buffer = postcard::to_extend(message, buffer)?;

        let payload_len = self.write_buffer.len() - HEADER_SIZE;
        if payload_len > MAX_FRAME_SIZE {
            bail!("incoming message too large: {} bytes", payload_len);
        }

        self.write_buffer[..HEADER_SIZE].copy_from_slice(&(payload_len as u32).to_be_bytes());

        self.writer.write_all(&self.write_buffer).await?;

        Ok(())
    }

    pub async fn receive<T>(&mut self) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let mut header = [0u8; HEADER_SIZE];

        match self.reader.read_exact(&mut header).await {
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(None);
            }
            Err(err) => return Err(err.into()),
        }

        let length = u32::from_be_bytes(header) as usize;
        error!("Reading {} bytes from the socket stream", length);

        if length > MAX_FRAME_SIZE {
            bail!("incoming message too large: {} bytes", length);
        }

        self.read_buffer.resize(length, 0);

        self.reader.read_exact(&mut self.read_buffer).await?;
        error!("Read {} bytes from socket to the buffer", self.read_buffer.len());

        let message = postcard::from_bytes(&self.read_buffer)?;

        Ok(Some(message))
    }
}
