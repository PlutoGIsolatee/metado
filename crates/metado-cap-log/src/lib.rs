//! Metado log 能力（Task 3.5）：info/warn/error/debug 日志写能力集。
//! 宿主可注入 sink 捕获日志（默认 stderr）。

use std::io::Write;

use metado_engine::capability::{CapabilityMeta, CapabilitySet};

pub struct MetaLog;

impl CapabilitySet for MetaLog {
    fn meta(&self) -> CapabilityMeta {
        CapabilityMeta {
            name: "log".into(),
            permissions: vec![
                "log.info".into(),
                "log.warn".into(),
                "log.error".into(),
                "log.debug".into(),
            ],
            exports: vec!["info".into(), "warn".into(), "error".into(), "debug".into()],
        }
    }
}

pub trait LogSink {
    fn write(&mut self, level: &str, message: &str);
}

/// 默认 sink：写 stderr（含级别前缀）。
pub struct StderrSink;

impl LogSink for StderrSink {
    fn write(&mut self, level: &str, message: &str) {
        let _ = writeln!(std::io::stderr(), "[{}] {}", level, message);
    }
}

pub struct Logger {
    sink: Box<dyn LogSink>,
}

impl Logger {
    pub fn new<S: LogSink + 'static>(sink: S) -> Self {
        Self {
            sink: Box::new(sink),
        }
    }

    pub fn info(&mut self, message: &str) {
        self.sink.write("info", message);
    }

    pub fn warn(&mut self, message: &str) {
        self.sink.write("warn", message);
    }

    pub fn error(&mut self, message: &str) {
        self.sink.write("error", message);
    }

    pub fn debug(&mut self, message: &str) {
        self.sink.write("debug", message);
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new(StderrSink)
    }
}