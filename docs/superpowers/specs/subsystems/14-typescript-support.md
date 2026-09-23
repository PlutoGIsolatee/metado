# TypeScript 支持设计 (v1.1+ 实现)

> **状态**：设计完整，**v1 不实现**，v1.1+ 实现
> **决策**：无增量编译、无类型检查、全量编译 + 内容哈希缓存、仅 transpile

---

## 设计决策

| 项目 | 决策 | 理由 |
|------|------|------|
| **增量编译** | ❌ 不做 | swc 无原生支持，自建成本高，v1 收益边际 |
| **类型检查** | ❌ 不做 (v1) | v1 仅 transpile，类型检查留待 v1.1+ |
| **编译模式** | 全量编译 + 内容哈希缓存 | 简单、可靠、单文件 < 100ms |
| **编译器** | swc (Rust) | 纯 Rust、极快、零外部依赖 |

---

## 编译流程

```
.mdl (纯 .ts 源码)
    ↓
实例化前即时编译
    ↓
swc 全量编译 (transpile only)
    ↓
内容哈希缓存 (SHA256 源码 + 配置哈希)
    ↓
生成 .js + SourceMap v3
    ↓
实例化执行
```

---

## 缓存设计

### 缓存键
```
CacheKey = SHA256(
    source_code + 
    swc_version + 
    swc_config_hash + 
    tsconfig_hash
)
```

### 缓存结构
```
~/.cache/metado/compile/
└── <plugin_id>/
    └── <hash_16chars>/
        ├── module.js        # 编译产物
        ├── module.js.map    # SourceMap v3
        └── meta.json        # { hash, swc_version, timestamp }
```

### 缓存策略
- **命中**：直接加载 `.js` + `.map`
- **未命中**：swc 全量编译 → 原子写入缓存 → 返回
- **失效**：源码变更、swc 版本升级、配置变更 → 自动重编译

---

## 编译配置

```rust
struct CompileConfig {
    swc_version: String,        // 锁定版本
    swc_config: SwcConfig,      // 编译选项
    tsconfig_hash: Option<String>, // 可选，用于缓存键
    target: SwcTarget::ES2022,  // 目标语言版本
    module: SwcModule::ESModule, // ESM 输出
}
```

### swc 关键配置
```json
{
  "jsc": {
    "target": "es2022",
    "parser": { "syntax": "typescript", "tsx": false, "decorators": false },
    "transform": { "legacyDecorator": false, "decoratorMetadata": false }
  },
  "module": { "type": "es6" },
  "sourceMaps": true,
  "minify": false
}
```

---

## 类型检查（v1.1+ 实现）

| 阶段 | 策略 |
|------|------|
| **v1** | 无类型检查，仅 transpile |
| **v1.1** | `mdl test` 强制 `tsc --noEmit`；`mdl build` 可选 `--typecheck` |
| **v1.2** | IDE 集成、增量类型检查、语言服务器 |

### v1.1 类型检查集成
```bash
# mdl test 时
mdl test --typecheck  # 强制 tsc --noEmit

# mdl build 可选
mdl build --typecheck  # 可选，swc + tsc --noEmit
```

---

## 编译缓存 API

```rust
trait CompileCache {
    fn get(&self, key: &CacheKey) -> Option<CompiledModule>;
    fn set(&self, key: CacheKey, module: CompiledModule) -> Result<()>;
    fn invalidate(&self, plugin_id: &str) -> Result<()>;
}

struct CompiledModule {
    js_code: String,
    source_map: String,  // SourceMap v3 JSON
    hash: String,        // 源码哈希
}
```

---

## 运行时编译流程

```rust
fn ensure_compiled(plugin: &Plugin, cache: &CompileCache) -> Result<CompiledModule> {
    let source = read_ts_source(&plugin);
    let key = cache_key(&source, &compile_config);
    
    // 1. 尝试缓存
    if let Some(cached) = cache.get(&key) {
        if cached.hash == hash_source(&source) {
            return Ok(cached);
        }
    }
    
    // 2. 全量编译
    let compiled = swc_compile(&source, &config)?;
    
    // 3. 写入缓存 (原子写入)
    cache.set(key, compiled.clone())?;
    
    Ok(compiled)
}
```

---

## 与现有系统集成

| 集成点 | 方式 |
|--------|------|
| `mdl build` | 仅打包签名，不编译 |
| `mdl run` | 实例化前编译 → 缓存 → 执行 |
| `mdl test` | 编译 + 可选类型检查 → 执行测试 |
| `mdl watch` | 文件变更 → 重新编译缓存 → 热重载 |

---

## 待补项 (v1.1+)

- [ ] swc 编译器集成 (编译期依赖)
- [ ] 编译缓存目录管理
- [ ] SourceMap v3 生成与加载
- [ ] 编译错误友好提示 (映射回 .ts 行号)
- [ ] 并发编译锁 / 编译队列
- [ ] 类型检查集成 (tsc --noEmit)
- [ ] 缓存清理策略 (LRU / 大小限制 / TTL)

---

## 不做项 (v1 明确不做)

- ❌ 增量编译
- ❌ 类型检查 (v1)
- ❌ swc 内置类型检查
- ❌ tsc --incremental
- ❌ 热重载时的增量重编译 (v1 全量重编译)
- ❌ 零拷贝 / 共享缓存 (进程间)