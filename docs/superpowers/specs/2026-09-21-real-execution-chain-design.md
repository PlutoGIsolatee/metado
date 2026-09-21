# 真实执行链路设计（最小链路）v1

日期：2026-09-21
分支：`feat/v1-engine`
状态：待评审（评审后锁定）

> 范围：评审总体判断「治理语义是占位而非实现」之执行主体。本设计把 `@metado/runtime`
> 合成模块从**形状 stub** 升级为**真实宿主 dispatch**——插件在 `mdl run` 下真实读写 storage、
> 打日志、取时间、生成随机字节，端到端跑真实代码。其余能力（http/file/sleep/custom）保持
> stub（宿主返回「not implemented (v1)」），后续逐类扩张。内置 API 本身保持基本功能，不再
> 硬化 cap 契约（上限/typed 错误另议）。

## 1. 目标与非目标

目标：

- 在 executor 内建立**唯一宿主入口**，插件经 `@metado/runtime` 命名空间方法调用真实宿主服务。
- `mdl run` 对含 storage/log/time.now/crypto.randomBytes 的插件产生真实结果，端到端可测试。
- 零回归：`PluginRuntime::new` 语义不变（host=None → 现 stub 行为），现有测试保持通过。
- 调用期放行在宿主侧二次裁决（纵深防御）。

非目标（本单元不做）：

- http / file / time.sleep / sha256 / hmac 真实化；`custom` 真实 handler。
- 异步非阻塞 dispatch（native fn 内同步执行，promise 形状返回已 settle 的 Promise）。
- daemon 侧接线与 `@metado/runtime` 形状改动。
- cap 容量上限与 typed 错误分类（用户已明确内置 API 保持基本功能，优先主体）。

## 2. 接线机制（已批准：方案 1 —— 单一 dispatch 桥）

保留 eval 形状工厂与 `spec_json`（规避 boa 0.22 逐方法 NativeFunction→JsValue 构造摩擦）。
放行的每个方法（`{"a": true}`）在形状工厂内改为：

```js
const mkHost = (ns, m, p) => (...a) =>
  p ? Promise.resolve(globalThis.__metadoDispatch(ns, m, ...a))
    : globalThis.__metadoDispatch(ns, m, ...a);
```

- 未放行方法保持现有 deny 语义（同步 throw / 异步 reject `PermissionDenied`）。
- host=None（`new` 路径）时方法为现有 stub 实现（no-op 成功），`__metadoDispatch` 不注册。
- promise 形状用 `Promise.resolve(...)` 包桥返回值：桥已返回 settle 后的 Promise 时，
  `Promise.resolve` 采纳其状态（reject 保持 reject），且桥本身不外抛，保证方法永不抛同步错误。

`__metadoDispatch` 为注册在 `globalThis` 的**唯一 NativeFunction 闭包**：
`(ns: string, method: string, ...args) => JsValue`。

- 内部调用 `HostDispatch::call(ns, method, args)`。
- 同步形状（`is_promise_style`=false）：`Ok(v)` 直接返回 `v`；`Err(m)` 同步 throw
  `Error(m)`（name=`PermissionDenied` 当且仅当错误以 `PermissionDenied:` 开头，否则 `CapabilityError`）。
- promise 形状：`Ok(v)` → 已 resolve 的 `JsPromise`（`JsPromise::resolve(v)`）；`Err(m)` →
  已 reject 的 `JsPromise`（能力错误 `Error(m)`，name 同上规则）。
- 参数：`JsValue → Value`（沿用 `js_to_value` 的单值转换；数组参数 v1 折叠规则即 `null`，
  与值模型一致——`storage.write(key, value)` 的 `value` 本单元仅支持字符串/数字/布尔/null）。

## 3. HostDispatch 接口（executor crate）

```rust
/// 宿主能力分派入口。executor 不持有任何内置实现，全部由宿主注入。
pub trait HostDispatch {
    /// 调用宿主能力；Err(message) 为能力失败（message 见 §4 name 规则）。
    fn call(&mut self, ns: &str, method: &str, args: &[Value]) -> Result<Value, String>;
}
```

`PluginRuntime` 变更（back-compat）：

- 新增 `PluginRuntime::new_with_host(exported, granted, surface, files, budget, host: Option<Box<dyn HostDispatch>>)`。
- 现有 `new(...)` / `new_with_instruction_budget(...)` 委托之，`host=None`（现行为零回归）。
- `PluginModuleLoader` 增 `dispatch: Option<Box<dyn HostDispatch>>` 字段；
  形状工厂据其存在性生成 host 方法或 stub 方法；`__metadoDispatch` 仅在 Some 时注册。

## 4. RealHost（cli crate，宿主侧实现）

```rust
pub struct RealHost {
    storage: cap_storage::Storage,
    granted: Vec<String>,   // 纵深防御：每次真实调用前 grants_allow 再裁决
    storage_dir: PathBuf,
}
```

`HostDispatch::call` 分派（母体不命中 → `Err("not implemented (v1): ns.method")`）：

| ns.method | 行为 |
|-----------|------|
| `storage.read(key)` | 先 `grants_allow(granted, "storage.read")` 否则 `Err(PermissionDenied: ...)`；调用 `storage.read(key)` 返回字节（UTF-8 校验失败→Fault；key 非法由 Storage 自身拒绝） |
| `storage.write(key, value)` | 先裁决 `storage.write`；`value` 串化规则：`String`→原样、`Number`→`to_string`、`Bool`→`"true"/"false"`、`Null`→空串，其余（v1 折叠为 null）→空串；写 `storage.write(key, bytes)` |
| `log.info/warn/error/debug(message)` | 裁决 `log.<level>`；`println!("[{level}] {message}")`（error→stderr） |
| `time.now()` | 裁决 `time.now`；返回 UNIX 毫秒数（`SystemTime`） |
| `crypto.randomBytes(n)` | 裁决 `crypto.randomBytes`；`cap_crypto::random_bytes(n)`（n 超 `usize`/负值 → Fault） |
| 其余 | `Err("not implemented (v1): http.get")` 等；custom 同理 |

- 构建：`RealHost::new(storage_dir: PathBuf, granted: Vec<String>)`；
  `run_mdl` 用 `METADO_STORAGE_DIR` env（缺省 `./metado-storage`）构造并传入
  `new_with_host(..., Some(Box::new(host)))`。
- 权限错误消息前缀 `PermissionDenied:` 由 §2 映射为 `PermissionDenied` 命名错误（与
  runtime-export-spec 拒绝语义一致）；其余真实失败为能力错误。

## 5. E2E 示例与验收

新增 `examples/storage-log/`（按 §6 决策 **b** 落地：`crypto.randomBytes(n)` 由宿主返回
十六进制字符串，同步形状，不破值模型）：

- `mdl.toml`：`permission = ["storage.read","storage.write","log.info","time.now","crypto.randomBytes"]`，
  `entry.boot.export = "boot"`。
- `src/main.js`：
  ```js
  import { storage, log, time, crypto } from '@metado/runtime';
  export default { boot() {
    const payload = `${time.now()}:${crypto.randomBytes(4)}`;
    storage.write('k', payload);
    const v = storage.read('k');
    log.info(v);
    return v;
  } };
  ```

验收测试 `crates/metado-cli/tests/run_real_test.rs`：

- 用现有测试辅助构造签名容器（`run_test.rs` 同款），指定临时 storage_dir；
- 断言 `RunOutcome.invoked == true` 且 `result` 为与写入等价的回环字符串
  （`<unix_millis>:<hex8>`，`time.now` 用正则校毫秒长，randomBytes 十六进制 8 字符）。

## 6. 待定项（spec 评审需锁定）

1. **crypto.randomBytes 返回值**（本栏锁定 **b**）：宿主编码为十六进制字符串返回
   （同步形状、不破值模型、可断言）。备选：
   a) 返回 `null`（只验证调用成功，长度不可得）；
   c) 推进值模型 Bytes 消歧（超出本单元最小范围）。
   待用户评审确认 b 或改选。
2. `storage.write` 第二参数支持类型为 §4 串化规则所列四类（字面均折叠）。
3. `log` 输出为 `[level] msg` 前缀，`log.error` → stderr，其余 stdout。
4. 缺少 manifest `boot` 入口时 `run_mdl` 现早返回（invoked=false），本单元不变。

## 7. 测试策略

- executor：`new_with_host` 用假 `HostDispatch`（记录接收的 ns/method/args，返回预设值）
  断言：(1) 放行方法经桥调用同步形状直接返回值；(2) 错误同步 throw；(3) promise 形状
  返回 settle 后的 Promise（reject 流为 Err）；(4) host=None 时行为与现 stub 一致（回归）；
  (5) `__metadoDispatch` 不存在于 host=None 时。
- cli：`RealHost` 单元测试（storage 回环、permission 二次裁决拒绝、randomBytes 串化、log 捕获）；
  `run_real_test.rs` 端到端。
- 回归：`cargo test --workspace --jobs 1` 全绿（现有 214 passed 为基线）。