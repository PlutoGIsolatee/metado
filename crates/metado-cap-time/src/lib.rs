//! Metado time 能力（Task 3.4）：`now()`（Unix 毫秒）与 `sleep(ms)`。
//! 宿主函数形态贴近 `Date.now()` 与 `Atomics.wait` 形状。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use metado_engine::capability::{CapabilityMeta, CapabilitySet};

pub struct MetaTime;

impl CapabilitySet for MetaTime {
    fn meta(&self) -> CapabilityMeta {
        CapabilityMeta {
            name: "time".into(),
            permissions: vec!["time.now".into(), "time.sleep".into()],
            exports: vec!["now".into(), "sleep".into()],
        }
    }
}

/// Unix 毫秒时间戳（Web Platform 形状）。
pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// 阻塞 sleep（v1 同步实现；异步运行时内为 Token 换接点）。
pub fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta() {
        let meta = MetaTime.meta();
        assert_eq!(meta.name, "time");
        assert_eq!(meta.permissions, vec!["time.now", "time.sleep"]);
        assert_eq!(meta.exports, vec!["now", "sleep"]);
    }
}