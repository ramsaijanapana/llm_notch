//! 4-byte big-endian length-prefixed frame IO with read timeouts.

use std::io::{Read, Write};
use std::time::Duration;

use interprocess::local_socket::tokio::prelude::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time;

use crate::error::{IpcError, IpcResult};
use crate::limits::{MAX_FRAME_BYTES, READ_TIMEOUT_MS};
use crate::wire::{WireMessage, decode_frame_bytes, encode_message};

pub fn read_frame_sync<R: Read>(reader: &mut R, timeout: Duration) -> IpcResult<WireMessage> {
    let mut len_buf = [0u8; 4];
    read_exact_with_timeout(reader, &mut len_buf, timeout)?;
    let length = u32::from_be_bytes(len_buf) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(IpcError::FrameRejected(format!(
            "declared length {length} exceeds max {MAX_FRAME_BYTES}"
        )));
    }
    let mut body = vec![0u8; length];
    read_exact_with_timeout(reader, &mut body, timeout)?;
    let mut frame = Vec::with_capacity(4 + length);
    frame.extend_from_slice(&len_buf);
    frame.extend_from_slice(&body);
    decode_frame_bytes(&frame)
}

pub fn write_frame_sync<W: Write>(writer: &mut W, message: &WireMessage) -> IpcResult<()> {
    let frame = encode_message(message)?;
    writer.write_all(&frame).map_err(IpcError::Io)?;
    writer.flush().map_err(IpcError::Io)?;
    Ok(())
}

pub async fn read_frame_async(stream: &mut LocalSocketStream) -> IpcResult<WireMessage> {
    let timeout = Duration::from_millis(READ_TIMEOUT_MS);
    time::timeout(timeout, read_frame_inner(stream))
        .await
        .map_err(|_| IpcError::ReadTimeout)?
}

pub async fn write_frame_async(
    stream: &mut LocalSocketStream,
    message: &WireMessage,
) -> IpcResult<()> {
    let frame = encode_message(message)?;
    stream.write_all(&frame).await.map_err(IpcError::Io)?;
    stream.flush().await.map_err(IpcError::Io)?;
    Ok(())
}

async fn read_frame_inner(stream: &mut LocalSocketStream) -> IpcResult<WireMessage> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(map_read_err)?;
    let length = u32::from_be_bytes(len_buf) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(IpcError::FrameRejected(format!(
            "declared length {length} exceeds max {MAX_FRAME_BYTES}"
        )));
    }
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).await.map_err(map_read_err)?;
    let mut frame = Vec::with_capacity(4 + length);
    frame.extend_from_slice(&len_buf);
    frame.extend_from_slice(&body);
    decode_frame_bytes(&frame)
}

fn read_exact_with_timeout<R: Read>(
    reader: &mut R,
    buf: &mut [u8],
    timeout: Duration,
) -> IpcResult<()> {
    if timeout.is_zero() {
        reader.read_exact(buf).map_err(map_read_err_sync)?;
        return Ok(());
    }
    let started = std::time::Instant::now();
    let mut offset = 0;
    while offset < buf.len() {
        if started.elapsed() > timeout {
            return Err(IpcError::ReadTimeout);
        }
        match reader.read(&mut buf[offset..]) {
            Ok(0) => return Err(IpcError::ConnectionClosed),
            Ok(n) => offset += n,
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(err) => return Err(IpcError::Io(err)),
        }
    }
    Ok(())
}

fn map_read_err(err: std::io::Error) -> IpcError {
    match err.kind() {
        std::io::ErrorKind::UnexpectedEof => IpcError::ConnectionClosed,
        _ => IpcError::Io(err),
    }
}

fn map_read_err_sync(err: std::io::Error) -> IpcError {
    map_read_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::IPC_WIRE_VERSION;
    use crate::wire::WireMessage;
    use std::io::Cursor;

    #[test]
    fn sync_round_trip() {
        let msg = WireMessage::Ack {
            v: IPC_WIRE_VERSION,
            request_id: "abc".into(),
        };
        let mut buf = Vec::new();
        write_frame_sync(&mut buf, &msg).expect("write");
        let mut cursor = Cursor::new(buf);
        let decoded = read_frame_sync(&mut cursor, Duration::from_secs(1)).expect("read");
        assert_eq!(decoded, msg);
    }
}
