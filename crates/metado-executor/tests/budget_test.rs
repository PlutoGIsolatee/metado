//! Task 2.4: Execution budget tests

use metado_executor::budget::ExecutionBudget;

#[test]
fn test_budget_consume() {
    let mut budget = ExecutionBudget::new(100);
    assert!(!budget.is_exhausted());
    budget.consume(50).unwrap();
    assert!(!budget.is_exhausted());
    // 50+51=101>100 → 超限 Err（计划原 60 无碍，但其 .unwrap() 是必 panic 的死代码）
    assert!(budget.consume(51).is_err());
    assert!(budget.is_exhausted());
}

#[test]
fn test_budget_exact() {
    let mut budget = ExecutionBudget::new(100);
    budget.consume(100).unwrap(); // 恰好花完 → 未耗尽
    assert!(!budget.is_exhausted());
    assert!(budget.consume(1).is_err()); // 任何超额 → Err（计划原文 .unwrap() 会 panic）
    assert!(budget.is_exhausted());
}

#[test]
fn test_budget_zero() {
    let mut budget = ExecutionBudget::new(0);
    assert!(budget.consume(0).is_ok());
    assert!(!budget.is_exhausted());
    assert!(budget.consume(1).is_err());
    assert!(budget.is_exhausted());
}