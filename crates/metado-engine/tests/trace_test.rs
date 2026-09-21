//! Task 1.9: Trace sink tests (§12.6)

use metado_engine::trace::{TraceEvent, TraceSink};

#[test]
fn test_record_and_retrieve() {
    let mut sink = TraceSink::new();
    sink.record(TraceEvent::CapabilityCall {
        module: "http".into(),
        line: 10,
        capability: "http.get".into(),
        exported: vec!["http".into()],
        granted: vec!["http.get".into()],
    });

    let events = sink.events();
    assert_eq!(events.len(), 1);
}

#[test]
fn test_filter_by_event_type() {
    let mut sink = TraceSink::new();
    sink.record(TraceEvent::EntryStart {
        entry: "onMessage".into(),
    });
    sink.record(TraceEvent::PermissionCheck {
        capability: "http.get".into(),
        passed: true,
    });
    sink.record(TraceEvent::EntryEnd {
        entry: "onMessage".into(),
    });

    let checks: Vec<_> = sink
        .events()
        .iter()
        .filter(|e| matches!(e, TraceEvent::PermissionCheck { .. }))
        .collect();
    assert_eq!(checks.len(), 1);
}

#[test]
fn test_value_flow_and_clear() {
    let mut sink = TraceSink::new();
    sink.record(TraceEvent::ValueFlow {
        direction: "in".into(),
        size_hint: 1024,
    });
    assert_eq!(sink.events().len(), 1);
    sink.clear();
    assert_eq!(sink.events().len(), 0);
}