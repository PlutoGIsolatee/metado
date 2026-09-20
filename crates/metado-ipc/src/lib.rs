//! Transport 层（§11）：消息级（帧化）收发，编解码与协议在其上构建。
//!
//! UnixListener 是 `mdl` 命令行工具需要的 accept 面；daemon 用 `unix_server` 循环 accept
//! 并逐连接驱动 Transport。Android/Windows 传输在宿主无法验证，见 plan 回写。

pub mod transport;
pub mod unix;

pub use transport::{IpcError, Transport, Result as IpcResult};
#[cfg(unix)]
pub use unix::{UnixListener, UnixTransport};