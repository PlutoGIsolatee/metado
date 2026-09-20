//! Unix Domain Socket 传输（Linux/macOS/Android-Termux）。帧协议见 transport.rs。

use std::os::unix::net::{UnixListener as StdListener, UnixStream as StdStream};
use std::path::{Path, PathBuf};

use crate::transport::{read_frame, write_frame, Result};
use crate::Transport;

/// 服务端：绑定路径、accept 连接、按连接驱动客户端。
#[allow(dead_code)]
pub struct UnixListener {
    inner: StdListener,
    path: PathBuf,
}

impl UnixListener {
    pub fn bind(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::remove_file(path);
        let inner = StdListener::bind(path)?;
        Ok(Self {
            inner,
            path: path.to_path_buf(),
        })
    }

    /// 阻塞接受一条连接。
    pub fn accept(&self) -> Result<(UnixTransport, std::os::unix::net::SocketAddr)> {
        let (stream, addr) = self.inner.accept()?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
        Ok((UnixTransport { inner: stream }, addr))
    }

    /// 关闭并移除 socket 文件。
    #[allow(dead_code)]
    pub fn shutdown(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Drop for UnixListener {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Unix socket 上的消息级传输。
pub struct UnixTransport {
    inner: StdStream,
}

impl UnixTransport {
    pub fn connect(path: &Path) -> Result<Self> {
        let stream = StdStream::connect(path)?;
        Ok(Self { inner: stream })
    }
}

impl Transport for UnixTransport {
    fn send(&mut self, payload: &[u8]) -> Result<()> {
        write_frame(&mut self.inner, payload)
    }

    fn receive(&mut self) -> Result<Vec<u8>> {
        read_frame(&mut self.inner)
    }

    fn close(&mut self) {
        let _ = self.inner.shutdown(std::net::Shutdown::Both);
    }
}

/// 服务端 accept 循环：对每条连接调用 `handle`（每连接一个 Transport 会话语义）。
pub fn unix_server(
    listener: &UnixListener,
    mut handle: impl FnMut(&mut UnixTransport),
) -> Result<()> {
    loop {
        let (mut conn, _addr) = listener.accept()?;
        handle(&mut conn);
        conn.close();
    }
}