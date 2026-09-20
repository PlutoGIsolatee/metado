//! Task 5.1: Transport trait + Unix Domain Socket（帧化消息语义）。

use std::sync::atomic::{AtomicUsize, Ordering};

use metado_ipc::{UnixListener, UnixTransport, Transport};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique_sock(tag: &str) -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut p = std::env::temp_dir();
    p.push(format!("metado-ipc-{}-{}-{}.sock", tag, std::process::id(), n));
    p
}

#[test]
fn test_unix_roundtrip() {
    let path = unique_sock("roundtrip");
    let listener = UnixListener::bind(&path).unwrap();
    let server_path = path.clone();
    let server = std::thread::spawn(move || {
        let (mut conn, _addr) = listener.accept().unwrap();
        let got = conn.receive().unwrap();
        assert_eq!(got, b"hello".to_vec());
        conn.send(b"world").unwrap();
        conn.close();
    });

    let mut client = UnixTransport::connect(&path).unwrap();
    client.send(b"hello").unwrap();
    let resp = client.receive().unwrap();
    assert_eq!(resp, b"world".to_vec());
    client.close();
    server.join().unwrap();
    let _ = std::fs::remove_file(&server_path);
}

#[test]
fn test_unix_multiple_frames_in_one_connection() {
    let path = unique_sock("multi");
    let listener = UnixListener::bind(&path).unwrap();
    let server_path = path.clone();
    let server = std::thread::spawn(move || {
        let (mut conn, _addr) = listener.accept().unwrap();
        for _ in 0..3 {
            let m = conn.receive().unwrap();
            conn.send(&m).unwrap();
        }
        conn.close();
    });

    let mut client = UnixTransport::connect(&path).unwrap();
    for i in 0..3u8 {
        let msg = vec![i; 100 + i as usize * 37];
        client.send(&msg).unwrap();
        let echo = client.receive().unwrap();
        assert_eq!(echo, msg);
    }
    client.close();
    server.join().unwrap();
    let _ = std::fs::remove_file(&server_path);
}

#[test]
fn test_unix_zero_length_and_large_payload() {
    let path = unique_sock("large");
    let listener = UnixListener::bind(&path).unwrap();
    let server_path = path.clone();
    let server = std::thread::spawn(move || {
        let (mut conn, _addr) = listener.accept().unwrap();
        let empty = conn.receive().unwrap();
        assert!(empty.is_empty());
        let big = conn.receive().unwrap();
        assert_eq!(big.len(), 512 * 1024);
        assert_eq!(big[0], 0xAB);
        assert_eq!(big[big.len() - 1], 0xCD);
        conn.send(b"done").unwrap();
        conn.close();
    });

    let mut client = UnixTransport::connect(&path).unwrap();
    client.send(b"").unwrap();
    let mut big = vec![0u8; 512 * 1024];
    big[0] = 0xAB;
    big[512 * 1024 - 1] = 0xCD;
    client.send(&big).unwrap();
    assert_eq!(client.receive().unwrap(), b"done".to_vec());
    client.close();
    server.join().unwrap();
    let _ = std::fs::remove_file(&server_path);
}

#[test]
fn test_unix_send_after_close_fails() {
    let path = unique_sock("closed");
    let listener = UnixListener::bind(&path).unwrap();
    let server_path = path.clone();
    let server = std::thread::spawn(move || {
        let (mut conn, _addr) = listener.accept().unwrap();
        conn.close();
    });

    let mut client = UnixTransport::connect(&path).unwrap();
    client.send(b"ping").unwrap();
    client.close();
    server.join().unwrap();
    assert!(client.send(b"late").is_err(), "send on closed transport must error");
    let _ = std::fs::remove_file(&server_path);
}