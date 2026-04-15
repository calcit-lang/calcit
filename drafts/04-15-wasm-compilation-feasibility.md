# WASM 编译可行性评估

## 目标

评估 Calcit 子集编译到 WASM 执行的可行性，分三个层面：

1. **解释器本身编译为 WASM**（在浏览器中运行 Calcit 解释器）
2. **Calcit 源码 AOT 编译到 WASM**（把 Calcit 程序直接翻译为 WASM 字节码）
3. **利用 WASM GC proposal** 的远期路径

## 依赖兼容性

### 核心依赖（Library crate）

| 依赖 | 用途 | WASM 兼容 | 说明 |
|------|------|----------|------|
| `rpds` 1.1 | Map/Set persistent DS | ✅ | 支持 `no_std`，纯 Rust |
| `im_ternary_tree` 0.0.20 | CalcitList 后端 | ✅ | 纯 Rust，无平台依赖 |
| `cirru_edn` | EDN 解析 | ✅ | 纯 Rust |
| `cirru_parser` | Cirru 语法解析 | ✅ | 纯 Rust |
| `rmp-serde` | MessagePack 序列化 | ✅ | 纯 Rust |

### CLI-only 依赖（已有或需要 cfg 门控）

| 依赖 | WASM | 现状 |
|------|------|------|
| `libloading` 0.9 | ❌ | 已 `cfg(not(wasm32))` 排除 |
| `ctrlc` 3.4 | ❌ | 已 `cfg(not(wasm32))` 排除 |
| `notify` 8.0 | ❌ | 仅 `src/bin/cr.rs` 使用，需新增门控 |
| `walkdir` 2.5 | ❌ | 仅 CLI 路径扫描，需新增门控 |
| `ureq` 3.2 | ⚠️ | 有 `wasm` feature 但需验证 |

**结论**: 核心数据结构和运行时无阻塞性依赖。

## 全局状态审计

WASM 是单线程的，以下全局状态需要关注：

### RwLock 包装（兼容，无需改动）

- `PROGRAM_CODE_DATA` — `LazyLock<RwLock<ProgramCodeData>>`
- `PROGRAM_RUNTIME_DATA_STATE` — `LazyLock<RwLock<ProgramRuntimeData>>`
- `PROGRAM_COMPILED_DATA_STATE` — `LazyLock<RwLock<ProgramCompiledData>>`
- `PROGRAM_DEF_ID_INDEX` — `LazyLock<RwLock<ProgramDefIdIndex>>`

WASM 单线程下 `RwLock` 退化为无竞争路径，开销极低。

### thread_local!（需评估）

| 位置 | 变量 | 影响 |
|------|------|------|
| `type_annotation.rs` | `TYPE_ANNOTATION_WARNING_CONTEXT` | WASM 下变全局，语义不变 |
| `type_annotation.rs` | `TYPE_SLOTS` | 同上 |
| `preprocess/mod.rs` | `PREPROCESS_COMPILE_GUARD` | 同上 |
| `preprocess/mod.rs` | `EXPECTED_FN_TYPE` | 同上 |
| `preprocess/mod.rs` | `EXPECTED_STRUCT_TYPE` | 同上 |
| `emit_js.rs` | `INLINE_ALL_ARGS` | codegen 专用，WASM target 可能不需要 |

**结论**: `thread_local!` 在 WASM 下退化为进程全局（因为只有一个线程），语义上没有破坏。不需要大规模重构，但如果追求整洁可改为显式参数传递（约 15 处）。

### AtomicBool（兼容）

- `CODEGEN_MODE`, `CODEGEN_SKIP_ARITY_CHECK` — 单线程下原子操作退化为普通读写。

## 路径一：解释器编译为 WASM

### 方案

- 目标: `wasm32-unknown-unknown`
- 仅编译 lib crate（排除 `src/bin/` 全部）
- 通过 `wasm-bindgen` 或 `wasm-pack` 导出 API
- 内嵌 `calcit-core.rmp` snapshot 作为 WASM 数据段（`include_bytes!` 天然支持）

### 导出接口设计

```rust
#[wasm_bindgen]
pub fn eval_calcit(code: &str) -> Result<String, String> {
    // 1. 加载内嵌 core snapshot
    // 2. 解析 code 为 Cirru
    // 3. 预处理 + 执行
    // 4. 返回序列化结果
}
```

### 需要的改动

1. 补充 `cfg` 门控：`notify`, `walkdir` 相关代码
2. `Cargo.toml` 添加 `[lib]` crate-type 含 `cdylib`
3. 添加 `wasm-bindgen` 可选依赖
4. 处理 `std::fs::read_to_string` 等 IO 调用（lib 内应已无直接调用，需验证）

### 工作量

小 — 主要是补门控和写胶水代码。

### 用途

- 浏览器端 Calcit REPL / playground
- 在线文档中的交互式代码示例
- 嵌入到 VS Code WebView 中执行

## 路径二：AOT 编译 Calcit 子集到 WASM

### 子集定义

能编译到 WASM 的 Calcit 特性：

```
✅ 可静态编译:
  - defn / fn（纯函数）
  - Record（defstruct / %{}）— 映射到线性内存 struct
  - Enum + tag-match — 映射到整数 tag + switch
  - Number / Bool / Tag — 映射到 WASM 原生类型
  - let 绑定
  - if / cond 条件
  - recur（尾递归 → WASM loop）
  - 算术运算（+, -, *, /, mod, 比较）

⚠️ 需要 host runtime bridge:
  - String 操作 — WASM 无原生字符串，需 host 分配
  - persistent Map / Set — 需 GC 或 host call
  - List 高阶操作（map / filter / foldl）— 回调需函数引用

❌ 排除（留给解释器 / JS codegen）:
  - 宏系统（编译期展开，不进入 WASM）
  - eval / quasiquote（需完整解释器）
  - 动态 method dispatch（无法静态解析）
  - Atom / Ref（可变状态需 host 端管理）
  - FFI / dylib 调用
```

### 编译策略

```
Calcit 源码
  → 预处理（宏展开 + 类型推导 + 重写优化）
  → IR（复用 gen_ir.rs 的 Cirru EDN IR）
  → emit_wasm.rs（新模块，IR → WAT/WASM）
  → .wasm 二进制
```

新增 `src/codegen/emit_wasm.rs` 与 `emit_js.rs` 并列：

- 函数 → `(func $name (param ...) (result ...) ...)`
- Record → 线性内存布局，字段按排序索引直接偏移
- Tag → 整数常量（编译期分配）
- tag-match → `br_table`（WASM 跳转表）
- let → `local.set` / `local.get`
- recur → `loop` + `br`

### 类型要求

**必须全局类型已知** — 这是 AOT 路径的硬前提：

- 所有函数签名完整（schema 覆盖）
- Record 字段类型确定（`CalcitStruct.field_types` 非 Dynamic）
- Enum variant payload 类型确定

当前类型系统已支持这些标注，但实际代码库覆盖率取决于用户代码。

### 内存模型

| Calcit 类型 | WASM 表示 | 大小 |
|-------------|----------|------|
| Number (f64) | f64 | 8 bytes |
| Bool | i32 (0/1) | 4 bytes |
| Tag | i32 (编译期整数) | 4 bytes |
| Nil | i32 (sentinel) | 4 bytes |
| Record | 线性内存 struct（字段连续排列） | Σ field sizes |
| Tuple | tag(i32) + payload(线性内存) | 4 + Σ payload sizes |
| String | host ref (externref / i32 handle) | 4 bytes |
| Map/Set/List | host ref | 4 bytes |

### 工作量

大 — 需要完整的类型推导验证、内存布局设计、WAT 代码生成、host bridge 规范。

## 路径三：WASM GC（远期）

### 背景

WASM GC proposal（2024 年起 V8/SpiderMonkey 已发布）提供：

- `struct.new` / `struct.get` — 结构体
- `array.new` / `array.get` — 数组
- `ref` 类型 — GC 自动管理的引用

### 映射

| Calcit | WASM GC |
|--------|---------|
| Record | `(type $Person (struct (field $age f64) (field $name (ref string))))` |
| List | `(type $List (array (ref any)))` |
| Tuple | `(type $Tuple (struct (field $tag i32) (field $extra (ref $List))))` |
| Map | 需要 host runtime 或自实现 hash trie |

### 优势

- 无需手动内存管理
- Persistent data structure 的 structural sharing 天然映射（GC 管理引用）
- 与 JS 互操作更自然（`externref` 直接传递）

### 工作量

取决于 WASM GC 生态成熟度。2026 年浏览器支持已基本到位，但工具链（assembler、debugger）仍在完善中。

## 可行性矩阵

| 维度 | 路径一（解释器→WASM） | 路径二（AOT→WASM） | 路径三（WASM GC） |
|------|---------------------|-------------------|-----------------|
| 语言覆盖 | 100%（完整解释器） | ~40%（纯函数子集） | ~70%（加 GC 后扩展） |
| 性能 | 中（解释开销） | 高（原生 WASM） | 高 |
| 工作量 | 小 | 大 | 大 |
| 类型要求 | 无 | 全量类型标注 | 大部分类型标注 |
| 依赖 | wasm-bindgen | 无额外 | WASM GC runtime |
| 适用场景 | playground/REPL | 热路径加速 | 通用 WASM 编译目标 |

## 建议路线

1. **近期**: 路径一 — 尝试 `cargo build --target wasm32-unknown-unknown --lib` 确认编译通过，排查剩余链接错误
2. **中期**: 路径二的子集 — 从纯数值计算函数开始，生成 WASM 模块作为 JS codegen 的"加速岛"（hot island），由 JS 运行时按需调用
3. **远期**: 路径三 — 随 WASM GC 工具链成熟，逐步扩展可编译子集

## 验证第一步

```bash
# 在 Cargo.toml 添加:
# [lib]
# crate-type = ["cdylib", "rlib"]

# 然后尝试:
cargo build --target wasm32-unknown-unknown --lib 2>&1 | head -50
```

输出的链接错误即为需要 `cfg` 门控的最终清单。
