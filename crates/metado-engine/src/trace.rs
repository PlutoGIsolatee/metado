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
        requested: Vec<String>,
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