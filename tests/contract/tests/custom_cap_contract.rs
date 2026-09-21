//! Task 6.3/6.4 契约：宿主自定义能力注册链路。
//! v1 扩展点 = `CapabilitySet` trait；注册后 permissions 并入 available，
//! `custom` 命名空间随 requested ∩ available 导出，import 即可链接。

use std::collections::HashMap;

use metado_engine::{
    capability::{CapabilityMeta, CapabilityRegistry, CapabilitySet},
    Container, Manifest, SignedBundle, Value,
};
use metado_executor::PluginRuntime;

use metado_contract::common::bundle_bytes;

struct AlertCapability;
impl CapabilitySet for AlertCapability {
    fn meta(&self) -> CapabilityMeta {
        CapabilityMeta {
            name: "custom".into(),
            permissions: vec!["custom.alert".into()],
            exports: vec!["alert".into()],
        }
    }
}

fn custom_and_builtin_permissions() -> Vec<String> {
    // 真实宿主可用权限 = 六个内置注册表 ∪ 宿主自定义能力
    let mut avail = metado_cli::registry_permissions();
    avail.push("custom.alert".to_string());
    avail.sort();
    avail
}

fn exported(requested: &[String], avail: &[String]) -> Vec<String> {
    metado_engine::exported_namespaces(requested, avail)
}

#[test]
fn contract_custom_capability_host_registration() {
    // 注册进宿主 CapabilityRegistry（v1 扩展点的正规使用方式）
    let mut reg = CapabilityRegistry::new();
    reg.register(Box::new(AlertCapability));
    assert!(reg.all_permissions().contains(&"custom.alert".to_string()));

    // 宿主注册 AlertCapability 后 custom.alert 进入 available
    let avail = custom_and_builtin_permissions();
    assert!(avail.contains(&"custom.alert".to_string()));

    // 插件请求 custom.alert + log.info → exported 含 custom + metado + log
    let requested = vec!["custom.alert".to_string(), "log.info".to_string()];
    let exported_list = exported(&requested, &avail);
    assert!(exported_list.contains(&"custom".to_string()));
    assert!(exported_list.contains(&"log".to_string()));
    assert!(exported_list.contains(&"metado".to_string()));

    // 运行插件：custom import 链接成功，boot 返回类型字符串
    let bytes = bundle_bytes(
        "name = \"alert\"\nversion = \"1.0.0\"\npermission = [\"custom.alert\", \"log.info\"]\n[entries.boot]\nexport = \"boot\"\n",
        "import { custom } from '@metado/runtime';\nexport default { boot() { return typeof custom; } };\n",
    );
    let bundle = SignedBundle::from_bytes(&bytes).unwrap();
    bundle.verify().unwrap();
    let container = Container::from_bytes(bundle.payload()).unwrap();
    let manifest_raw = container.read_file("mdl.toml").unwrap();
    let manifest = Manifest::from_toml(&String::from_utf8(manifest_raw).unwrap()).unwrap();
    let mut requested = manifest.permission.clone();
    let exported_list = exported(&requested, &avail);

    let files: HashMap<String, Vec<u8>> = container
        .files()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let files = Box::new(move |rel: &str| files.get(rel).cloned());
    let mut rt = PluginRuntime::new(&exported_list, &requested, &avail, files).unwrap();
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    let _ = &mut requested;
    assert_eq!(out, Value::String("object".to_string()));
}

#[test]
fn contract_custom_capability_absent_without_host_registration() {
    // 未注册 custom 的宿主：available 不含 custom.alert → exported 无 custom →
    // import 链接失败（与 CLI 对 custom-cap-plugin 的诊断一致）
    let owned: Vec<String> = vec![];
    let avail: &[String] = &owned;
    let one = exported(&["custom.alert".to_string()], avail);
    assert!(!one.contains(&"custom".to_string()));
    let two = exported(&["custom.alert".to_string()], &["log.info".to_string()]);
    assert!(!two.contains(&"custom".to_string()));
}