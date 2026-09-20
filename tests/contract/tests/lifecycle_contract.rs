//! Task 6.4 契约：生命周期状态机（§6）。

use metado_engine::{PluginState, Plugin};

#[test]
fn contract_normal_install_chain_permitted() {
    let mut p = Plugin::new("demo", "signer");
    p.transition(PluginState::Installing).unwrap();
    p.transition(PluginState::Installed).unwrap();
    p.transition(PluginState::Loading).unwrap();
    p.transition(PluginState::Pending).unwrap();
    p.transition(PluginState::Active).unwrap();
    assert_eq!(p.state, PluginState::Active);
}

#[test]
fn contract_evicted_can_reenter_active() {
    let mut p = Plugin::new("demo", "signer");
    p.transition(PluginState::Installing).unwrap();
    p.transition(PluginState::Installed).unwrap();
    p.transition(PluginState::Loading).unwrap();
    p.transition(PluginState::Pending).unwrap();
    p.transition(PluginState::Active).unwrap();
    p.transition(PluginState::Evicted).unwrap();
    p.transition(PluginState::Active).unwrap();
    assert_eq!(p.state, PluginState::Active);
}

#[test]
fn contract_uninstalled_is_terminal_from_any_state() {
    for from in [
        PluginState::Absent,
        PluginState::Installing,
        PluginState::Installed,
        PluginState::Loading,
        PluginState::Pending,
        PluginState::Active,
        PluginState::Evicted,
        PluginState::Quarantined,
    ] {
        let mut p = Plugin::new("demo", "signer");
        p.state = from;
        p.transition(PluginState::Uninstalled).unwrap();
        assert_eq!(p.state, PluginState::Uninstalled);
    }
}

#[test]
fn contract_illegal_transitions_rejected() {
    let mut p = Plugin::new("demo", "signer");
    // Active → Installing 非法
    p.state = PluginState::Active;
    assert!(p.transition(PluginState::Installing).is_err());
    // Uninstalled 为终态
    p.state = PluginState::Uninstalled;
    assert!(p.transition(PluginState::Active).is_err());
    assert_eq!(p.state, PluginState::Uninstalled, "rejected transition must leave state unchanged");
}

#[test]
fn contract_engine_load_rejects_tampered_bundle() {
    // 签名在 load 前置校验（bundle.verify）
    let mut bytes = common_helpers::bundle();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    let bundle = metado_engine::SignedBundle::from_bytes(&bytes).unwrap();
    let mut engine = metado_engine::Engine::new();
    assert!(engine.load_plugin(&bundle, vec![]).is_err());
}

mod common_helpers {
    pub fn bundle() -> Vec<u8> {
        use std::io::Write;
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zw = zip::ZipWriter::new(cursor);
        let opts = zip::write::SimpleFileOptions::default();
        zw.start_file("mdl.toml", opts).unwrap();
        zw.write_all(b"name = \"x\"\nversion = \"1.0.0\"\n").unwrap();
        let cursor = zw.finish().unwrap();
        metado_engine::SignedBundle::sign(
            &metado_engine::KeyPair::generate(),
            &cursor.into_inner(),
        )
        .to_bytes()
    }
}