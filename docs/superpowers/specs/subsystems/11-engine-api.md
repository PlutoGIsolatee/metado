# Metado 引擎 API 面

## 13. 引擎 API 面（Rust，示意）—— 引擎内部 / 构建集成面

> 本 Rust API 是**引擎进程内部**与**宿主开发者构建期**使用的面（CLI / 测试 / 引擎 daemon 主程序 / 定制能力编译）；运行期的**宿主**只面对 §11 的 IPC 协议，不直接调这套 API。

```rust
let mut engine = Engine::new();
engine.register_capability(Http::default());        // / storage / file / time / log / crypto
engine.define_permission_set("standard", ["http.get", "log.info"]);

let plugin = engine.load_plugin("plugin.mdl").await?;   // 验签闸门 → 解析 → 静态检查 → 权限注入
let signer = plugin.signer_id();                        // 稳定发布者身份（公钥指纹）
let mode   = plugin.lifecycle();                        // 读 effective 常驻模式（宿主经 RPC 设）
engine.grant(&plugin, ["http.get.api.example"]);        // 构建期/CLI 直接授予（生产宿主经 RPC grant）

let out = plugin.invoke("onMessage", value).await?;
// Result<Value, ExecutionError>
```

更新验签：引擎对比**当前存储本体**的 signer 与新 bundle 的 signer（自本体重算比对），引擎为唯一写者、本体只能经引擎入位，**无须持久化首装指纹锚**（§6.2-3 修订）。