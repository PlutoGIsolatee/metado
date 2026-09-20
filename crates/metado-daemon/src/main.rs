//! metado-daemon：Engine 进程。监听 Unix socket，逐连接 JSON-RPC 分发到引擎。
//! 用法：metado-daemon --socket PATH

use std::path::PathBuf;

use metado_daemon::Daemon;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let socket = match parse_socket(&args) {
        Ok(p) => p,
        Err(m) => {
            eprintln!("usage: metado-daemon --socket PATH\n{}", m);
            std::process::exit(2);
        }
    };

    let mut daemon = Daemon::new();
    if let Err(e) = daemon.serve_unix(&socket) {
        eprintln!("daemon: {}", e);
        std::process::exit(1);
    }
}

fn parse_socket(args: &[String]) -> Result<PathBuf, String> {
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        if a == "--socket" {
            return Ok(PathBuf::from(
                it.next().ok_or_else(|| "missing value after --socket".to_string())?,
            ));
        }
    }
    Err("--socket PATH is required".to_string())
}