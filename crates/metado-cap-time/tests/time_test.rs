//! Task 3.4: metado-cap-time tests

use metado_cap_time::MetaTime;
use metado_engine::capability::CapabilitySet;

#[test]
fn test_meta() {
    let meta = MetaTime.meta();
    assert_eq!(meta.name, "time");
    for perm in ["time.now", "time.sleep"] {
        assert!(meta.permissions.contains(&perm.to_string()), "missing {}", perm);
    }
    for exp in ["now", "sleep"] {
        assert!(meta.exports.contains(&exp.to_string()), "missing {}", exp);
    }
}

#[test]
fn test_now_positive() {
    // 当前实现返回 Unix 毫秒
    assert!(metado_cap_time::now_ms() > 1_000_000_000);
}

#[test]
fn test_sleep_duration() {
    let t0 = metado_cap_time::now_ms();
    metado_cap_time::sleep_ms(20);
    let dt = metado_cap_time::now_ms() - t0;
    assert!(dt >= 15 && dt < 2000);
}