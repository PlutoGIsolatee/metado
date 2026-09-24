# Metado 打包与构建

## 10. 打包与构建

### 10.1 源码（合法 Node 项目，标准 JS 结构）

插件源码是一个**合法 Node 项目**（除 API，§2）：结构、约定与工具链兼容 npm 生态，运行时 API 由 metado 治理能力面代替 Node 核心 API。

```
myplugin/
  package.json          # name/version/"type":"module"/exports
  package-lock.json     # 依赖快照（可复现构建）
  node_modules/         # npm install 产出，真实 npm 依赖
  mdl.toml              # manifest：name/version/permission/permission-set/entries/lifecycle
  src/
    on_message.js       # 标准 ESM：export async function onMessage(input){...}
    helpers/util.js     # 内部模块，自由组织
  wasm/
    transform.wasm      # 计算内核原始字节（.wat 由作者侧工具先行编译，非引擎转换）
```

- 结构 = 标准 npm 项目：`npm install` 直接工作，编辑器/lint/LSP/bundler 全部可用
- 唯一例外是运行时 API：**Node 核心内置模块不可用**（process/fs/net/child_process/require），一律以受治理的 metado 能力替代；仅白名单 shim（`Buffer`、`path`、`events`）
- 运行时模块解析为 **Node 风格**：`exports` field 优先、`node_modules` 逐级查找（§9.1/阶段 2）

### 10.2 分发（单文件，签名）

**plugin.mdl = 签名信封 + ZIP 模块树容器**

```
plugin.mdl =
  签名信封
    header       magic "MDL1" + format_version(u16) + algorithm("ed25519")
                 + signer_pubkey(32B) + payload_len(u64)
    signature    Ed25519(64B)，覆盖 header 之后全部字节（验签闸门在解包前，§6.2）
    payload      ZIP 模块树容器

payload = ZIP（entry = 文件，路径 = 容器内相对路径；v1 全 store，不压缩）
  mdl.toml           # manifest：name/version/permission/permission-set/entries/lifecycle
  src/**             # ESM 模块，原样字节
  node_modules/**    # npm 依赖，原样字节
  wasm/**            # WASM 计算内核，原样字节
  rotation-entry     # 仅轮换过渡包：旧钥签署的迁移背书独立条目（与 mdl.toml 并列，不混入 manifest；条目名实现定；语义见 §6.4）
```

- **索引 = ZIP central directory**：任意模块按 path O(1) 定位 offset/length，无需自写二进制索引表；**入口表 = manifest（entries）**，容器内无冗余索引
- **工具链可检视**：`unzip`/标准工具直接打开（契合"合法 Node 项目"），生产调试仍读加载后的文件树与真实行号
- **免转义**：wasm / 任意字节原样入 entry，零 base64
- **零转换**：store 即原样字节；压缩推迟（§15），将来 per-entry DEFLATE 不破坏格式（header 含 format_version 供演进）
- **挂载安全**：只读模块文件系统，路径按容器内相对路径解析，防 `../` 穿越
- `mdl build`：组装 ZIP 容器（源码树原样装入）→ `mdl sign --key <file>` 签名 → 单文件 `plugin.mdl`

### 10.3 生态复用（npm 包）

- **插件就是合法 Node 项目**，npm 依赖是标准手段：`npm install` 产出 node_modules，`mdl build` 按 `package-lock.json` 快照把依赖拷入容器（可复现），不手动管理
- 纯 ES module JS 包可拷入容器（lodash-es、axios、zod 等），模块原样保留
- **不做自动 `require()`→ESM 重写**；CJS-only 包需作者在自有工具链中预转换（否则不支持）
- 提供轻量 Node API shim（`Buffer`、`path`、`events`）
- 依赖 Node 原生模块的包不可用，必须用宿主 API 替代（`import { storage, http } from "@metado/runtime"`）