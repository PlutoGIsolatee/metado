use metado_engine::Value;

use crate::budget::ExecutionBudget;
use crate::engine::JsEngine;
use crate::virtual_module::VirtualModule;

/// 执行器门面：串起 JsEngine + @metado/runtime 虚拟模块 + 执行预算。
/// v1 预算按"边界操作"（每次 eval/call 记 1）消费；将来接入 boa 指令级
/// 燃料时替换内部实现，公开 API 不变。
pub struct Executor {
    engine: JsEngine,
    budget: ExecutionBudget,
    virtual_module: VirtualModule,
    ops_per_operation: u64,
}

impl Executor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_budget(max_ops: u64) -> Self {
        Self {
            engine: JsEngine::new(),
            budget: ExecutionBudget::new(max_ops),
            virtual_module: VirtualModule::build(Vec::new(), Vec::new()),
            ops_per_operation: 1,
        }
    }

    pub fn eval(&mut self, code: &str) -> Result<Value, String> {
        self.budget.consume(self.ops_per_operation)?;
        self.engine.eval(code)
    }

    pub fn call(&mut self, entry: &str, args: Vec<Value>) -> Result<Value, String> {
        self.budget.consume(self.ops_per_operation)?;
        self.engine.call(entry, args)
    }

    pub fn set_virtual_module(&mut self, vm: VirtualModule) {
        self.virtual_module = vm;
    }

    pub fn has_export(&self, name: &str) -> bool {
        self.virtual_module.has_export(name)
    }

    pub fn is_granted(&self, capability: &str) -> bool {
        self.virtual_module.is_granted(capability)
    }

    pub fn budget(&self) -> &ExecutionBudget {
        &self.budget
    }
}

impl Default for Executor {
    fn default() -> Self {
        // 默认预算 10_000_000，足够常规插件开发调试
        Self::new_with_budget(10_000_000)
    }
}