use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum TraceEvent {
    EntryStart {
        entry: String,
    },
    EntryEnd {
        entry: String,
    },
    CapabilityCall {
        module: String,
        line: u32,
        capability: String,
        /// 该事件签发时的导出命名空间面（非真实 permission-requested 上界）。
        exported: Vec<String>,
        granted: Vec<String>,
    },
    PermissionCheck {
        capability: String,
        passed: bool,
    },
    ValueFlow {
        direction: String, // "in" or "out"
        size_hint: usize,
    },
}

pub struct TraceSink {
    events: Vec<TraceEvent>,
}

impl TraceSink {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn record(&mut self, event: TraceEvent) {
        self.events.push(event);
    }

    pub fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}