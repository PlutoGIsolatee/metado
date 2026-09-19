//! Task 3.5: metado-cap-log tests

use std::sync::{Arc, Mutex};

use metado_cap_log::{LogSink, Logger, MetaLog};
use metado_engine::capability::CapabilitySet;

struct ArcSink {
    buf: Arc<Mutex<Vec<String>>>,
}

impl LogSink for ArcSink {
    fn write(&mut self, level: &str, message: &str) {
        self.buf
            .lock()
            .unwrap()
            .push(format!("[{}] {}", level, message));
    }
}

#[test]
fn test_meta() {
    let meta = MetaLog.meta();
    assert_eq!(meta.name, "log");
    for perm in ["log.info", "log.warn", "log.error", "log.debug"] {
        assert!(meta.permissions.contains(&perm.to_string()), "missing {}", perm);
    }
    for exp in ["info", "warn", "error", "debug"] {
        assert!(meta.exports.contains(&exp.to_string()), "missing {}", exp);
    }
}

#[test]
fn test_logger_records_with_level() {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let mut logger = Logger::new(ArcSink { buf: buf.clone() });
    logger.info("hello");
    logger.debug("dbg");
    logger.error("boom");
    assert_eq!(
        *buf.lock().unwrap(),
        vec!["[info] hello", "[debug] dbg", "[error] boom"]
    );
}