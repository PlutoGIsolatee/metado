# Metado 签名与信任

## 6. 签名与信任（类 Android 模型）

### 6.1 机制

- **算法：Ed25519（v1 唯一）**；签名 64 字节，验签快，Rust 生态成熟。无 RSA/ECDSA/X.509/证书链 —— 信任判定在引擎外，PKI/CA 无必要
- **信封格式**：

```
.mdl = header { magic "MDL1", format_version, signer_pubkey, algorithm = "ed25519" }
     + signature（覆盖后续所有字节）
     + payload（ZIP 模块树容器：manifest + src + node_modules + wasm，§10.2）
```

- **`signer_id` = 公钥指纹**（如 sha256 截段），作为插件的稳定发布者身份

### 6.2 语义（复制 Android 结构，简化机制）

1. **载荷前独立验签闸门**：`load_plugin` 先验签（甚至不解包 payload），失败即拒
2. **签名 = 稳定身份**：signer_id 绑定插件身份
3. **同签更新（引擎强制）**：同 plugin_id 的新 bundle，`signer_pubkey` 指纹必须等于**当前存储本体**的 signer（自本体重算比对），否则拒绝 —— 防冒名覆盖，不依赖用户决策；引擎为唯一写者，本体只能经引擎入位，无须持久化首装锚
4. **引擎不裁决可信性**：只做完整性验证 + 身份比对；发布者可不可信由用户/宿主决定（对应 Android 的渠道/侧载）
5. **私钥永不入引擎**：签名私钥只在插件开发者侧，文件只含公钥与指纹

### 6.3 同签名互通（显式 opt-in）

- 以 signer 域划分存储/vfs 命名空间的第二层：

```
storage keyspace:
  私有   storage:<plugin_id>:<key>      ← 本插件专属
  共享   storage:<signer_id>:<key>      ← 同发布者（同签）共享

权限声明显式 opt-in：
  storage.<signer>.write / vfs.<signer>.read
  未声明 → 共享域不可达；同签插件默认不互通，最小权限保持
```

- 引擎需在解包验签后、权限注入前完成 `<signer>` 绑定
- 共享域基于签名验证后的 signer_id，不可伪造（验签是前置闸门）