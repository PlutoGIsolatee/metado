//! `mdl trace`（Task 4.7）：执行轨迹观测（§12.6）。
//! - 运行插件（entries.test → test；否则 boot 冒烟）并收集轨迹事件
//! - 人类可读树 / 按事件类型过滤 / JSON（TraceEvent 已 Serialize）

use std::collections::HashMap;

use metado_engine::{Container, Manifest, SignedBundle, TraceEvent};
use metado_executor::PluginRuntime;

use crate::env::{exported_namespaces, registry_permissions};

/// 附查看器的轨迹事件。
#[derive(Debug, Clone)]
pub struct TraceLine(pub TraceEvent);

impl TraceLine {
    pub fn kind(&self) -> &'static str {
        match &self.0 {
            TraceEvent::EntryStart { .. } => "EntryStart",
            TraceEvent::EntryEnd { .. } => "EntryEnd",
            TraceEvent::CapabilityCall { .. } => "CapabilityCall",
            TraceEvent::PermissionCheck { .. } => "PermissionCheck",
            TraceEvent::ValueFlow { .. } => "ValueFlow",
        }
    }

    pub fn capability(&self) -> &str {
        match &self.0 {
            TraceEvent::CapabilityCall { capability, .. } => capability,
            TraceEvent::PermissionCheck { capability, .. } => capability,
            _ => "",
        }
    }

    pub fn detail(&self) -> String {
        match &self.0 {
            TraceEvent::EntryStart { entry } => format!("entry {}", entry),
            TraceEvent::EntryEnd { entry } => format!("entry {} done", entry),
            TraceEvent::CapabilityCall { module, line, requested, .. } => {
                format!("{}@{} requested {}", module, line, requested.join(", "))
            }
            TraceEvent::PermissionCheck { capability, passed } => {
                format!("{} {}", capability, if *passed { "granted" } else { "denied" })
            }
            TraceEvent::ValueFlow { direction, size_hint } => {
                format!("flow {} ({} item(s))", direction, size_hint)
            }
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        // TraceEvent 已 serialize：按其结构输出，另附 kind 便于过滤/分组
        let mut obj = serde_json::to_value(&self.0).unwrap_or(serde_json::Value::Null);
        if let serde_json::Value::Object(m) = &mut obj {
            m.insert("kind".into(), serde_json::Value::String(self.kind().into()));
        }
        obj
    }
}

/// 运行插件收集轨迹；`filter`（Some(类型名)）只保留该类型事件。
pub fn trace_mdl(
    bytes: &[u8],
    extra_grant: &[String],
    filter: Option<&str>,
) -> Result<Vec<TraceLine>, String> {
    let bundle = SignedBundle::from_bytes(bytes)?;
    bundle.verify()?;

    let container = Container::from_bytes(bundle.payload())?;
    let manifest_raw = container.read_file("mdl.toml")?;
    let manifest_raw = String::from_utf8(manifest_raw).map_err(|e| format!("manifest utf8: {}", e))?;
    let manifest = Manifest::from_toml(&manifest_raw).map_err(|e| format!("manifest: {}", e))?;

    let available = registry_permissions();
    let mut requested = manifest.permission.clone();
    requested.extend(extra_grant.iter().cloned());
    let exported = exported_namespaces(&requested, &available);

    let files: HashMap<String, Vec<u8>> = container
        .files()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let files = Box::new(move |rel: &str| files.get(rel).cloned());

    let mut rt = PluginRuntime::new(&exported, files)?;
    let events: std::rc::Rc<std::cell::RefCell<Vec<TraceEvent>>> =
        std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink_events = events.clone();
    let sink = Box::new(move |ev: TraceEvent| sink_events.borrow_mut().push(ev));
    rt.set_trace(sink);
    rt.load("src/main.js")
        .map_err(|e| format!("load entry src/main.js: {}", e))?;

    let case = manifest
        .entries
        .get("test")
        .map(|e| ("test".to_string(), e.export.clone()))
        .or_else(|| {
            manifest
                .entries
                .get("boot")
                .map(|e| ("boot".to_string(), e.export.clone()))
        });

    if let Some((_name, export)) = case {
        let _ = rt.call_default(&export, vec![]);
    }

    let out: Vec<TraceLine> = events
        .borrow().clone()
        .into_iter()
        .map(TraceLine)
        .filter(|l| filter.map_or(true, |f| l.kind() == f))
        .collect();
    Ok(out)
}