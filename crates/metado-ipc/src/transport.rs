use std::io;

/// IPC 层统一错误。
#[derive(Debug)]
pub enum IpcError {
    Io(io::Error),
    Closed,
    Overlength,
    Protocol(String),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::Io(e) => write!(f, "io: {}", e),
            IpcError::Closed => write!(f, "transport closed"),
            IpcError::Overlength => write!(f, "frame exceeds length limit"),
            IpcError::Protocol(m) => write!(f, "protocol: {}", m),
        }
    }
}

impl std::error::Error for IpcError {}

impl From<io::Error> for IpcError {
    fn from(e: io::Error) -> Self {
        IpcError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, IpcError>;

/// 消息级传输：`send`/`receive` 各收发一条完整消息（帧化由具体传输负责）。
pub trait Transport {
    fn send(&mut self, payload: &[u8]) -> Result<()>;
    fn receive(&mut self) -> Result<Vec<u8>>;
    fn close(&mut self);
}

/// 单一连接的帧边界：4-byte 大端长度前缀（上限 16 MiB，防放大）。
pub const FRAME_MAX: u32 = 16 * 1024 * 1024;

/// 工具：写一条帧。字节顺序大端。
pub fn write_frame<W: io::Write>(w: &mut W, payload: &[u8]) -> Result<()> {
    if payload.len() as u64 > FRAME_MAX as u64 {
        return Err(IpcError::Overlength);
    }
    let len = (payload.len() as u32).to_be_bytes();
    w.write_all(&len)?;
    w.write_all(payload)?;
    Ok(())
}

/// 工具：读一条帧。EOF 于帧边界 = 对端关闭连接（Closed）。
pub fn read_frame<R: io::Read>(r: &mut R) -> Result<Vec<u8>> {
    let mut header = [0u8; 4];
    match r.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Err(IpcError::Closed),
        Err(e) => return Err(IpcError::Io(e)),
    }
    let len = u32::from_be_bytes(header);
    if len > FRAME_MAX {
        return Err(IpcError::Overlength);
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)?;
    Ok(buf)
}