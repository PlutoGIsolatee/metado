# Metado 权限模型

## 5. 权限模型（核心）

三层要素：

```
① 请求（manifest 声明）           ② 授予（宿主/用户运行时决定）              ③ 执行（每次调用检查）
permission = [...]                 engine.grant(plugin, [...])             host_api 调用 →
  [http.get.api.example,             // granted ⊆ requested                    resolver.check(
  storage.<signer>.write]            // 用户可削减，领域规则叠加                    requested, granted,
                                      // engine.define_permission_set(...)         domain_rules) → pass/deny
```

### 5.1 权限语法（点分式）

- 细粒度点分式：`http.get`、`http.get.api.example`、`storage.read.profile`、`log.info`
- **signer 域引用**：`storage.<signer>.write`、`vfs.<signer>.read` 等 —— `<signer>` 在加载期绑定为该插件实际的 signer_id（§6），构成**同签名互通**的显式授权面
- 声明位置：manifest 顶层 `permission` 字段（插件级）

### 5.2 三层语义

- **最小权限**：realm 授权视图（加载时解析）= `requested ∩ available`；未请求的能力导出不存在（访问即快速失败）。静态可审计，默认拒绝
- **用户可设**：`granted ⊆ requested`，运行时由宿主应用决定实际授予范围；每次宿主 API 调用经运行时执行层复核
- **领域规则**：宿主可注册领域级 resolver 叠加（如"http 仅限本应用白名单域名"、"调用次数超限自动降级"、"单次载荷 Bytes/元素上限"）

### 5.3 权限集

- 宿主应用定义命名权限集，插件引用：

```rust
engine.define_permission_set("standard", ["http.get", "log.info"]);
engine.define_permission_set("finance",  ["http.get.api.bank", "storage.read", "crypto.sign"]);
```

- 插件 manifest：`permission-set = ["standard", "finance"]`
- 插件内联定义权限集仅用于内部复用
- 加载期检查：引用的权限集必须已由宿主定义，否则加载失败