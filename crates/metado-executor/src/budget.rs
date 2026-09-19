/// 燃料式执行预算：消费超过 max_ops 即算耗尽（is_exhausted），并返回 Err。
pub struct ExecutionBudget {
    max_ops: u64,
    consumed: u64,
}

impl ExecutionBudget {
    pub fn new(max_ops: u64) -> Self {
        Self {
            max_ops,
            consumed: 0,
        }
    }

    pub fn consume(&mut self, ops: u64) -> Result<(), String> {
        self.consumed += ops;
        if self.consumed > self.max_ops {
            Err("execution budget exhausted".into())
        } else {
            Ok(())
        }
    }

    pub fn is_exhausted(&self) -> bool {
        self.consumed > self.max_ops
    }
}