# WASM Codegen — 数据结构支持

## 背景

现有 WASM codegen（`emit_wasm.rs`）采用 all-f64 策略，仅支持纯数值函数。本文档描述如何在保持 all-f64 ABI 兼容的前提下，增量引入 Tag、Record、Tuple/Enum 等结构化数据的支持。

## 设计决策（继承自初始实现）

1. **二进制 `.wasm` 输出** — 使用 `wasm-encoder` crate 直接生成标准 WASM 二进制格式。
2. **All-f64 类型策略** — 所有值统一为 f64，Bool 用 1.0/0.0 表示。
3. **比较运算的 f64 返回** — 使用 `select` 指令将 i32 条件转为 f64 (1.0/0.0)。
4. **recur 映射到 WASM loop** — 通过 `block_depth` 追踪嵌套深度。

## 核心思路：线性内存 + f64 编码指针

### 值表示

保持所有函数参数和返回值为 f64，扩展值的语义：

| 值类型 | f64 表示方式 | 说明 |
|--------|-------------|------|
| Number | 直接 f64 | 不变 |
| Bool | 1.0 / 0.0 | 不变 |
| Nil | 0.0 | 不变 |
| Tag | 正整数 f64 (1.0, 2.0, ...) | 编译时分配，全局唯一 |
| Record 指针 | f64 编码的 i32 偏移 | 指向线性内存中的 Record 数据 |
| Tuple 指针 | f64 编码的 i32 偏移 | 指向线性内存中的 Tuple 数据 |

指针与数值之间的转换：
- 写入: `i32` → `f64.convert_i32_u`
- 读取: `f64` → `i32.trunc_f64_u`

### 歧义规避

Tag、指针、数值共用 f64 空间，依赖**编译时类型信息**区分：
- WASM 子集要求 Record/Tuple 相关函数必须有类型标注
- Tag 值与数值范围不重叠（Tag 从高位整数开始分配，或使用特殊编码）
- 实验阶段不做运行时类型检查，类型错误由 Calcit 预处理保证

## Tag 编译

Tag 在 Calcit 中是轻量标识符（如 `:ok`、`:err`、`:name`）。

### 方案

- 编译阶段遍历所有被使用的 Tag，分配从 `1.0` 开始的递增整数
- `Calcit::Tag(t)` 在 `emit_expr` 中编译为 `f64.const <tag_id>`
- Tag 比较复用现有 `f64.eq` 逻辑（`&=` 已支持）

### 编译上下文扩展

```rust
struct WasmGenCtx {
    // ... existing fields ...
    tag_index: HashMap<String, u32>,  // tag name → integer ID
    next_tag_id: u32,                 // counter, starts at 1
}
```

## Record 编译

### 内存布局

Record 在线性内存中按固定布局存储：

```
offset + 0:  struct_tag (f64, 8 bytes) — 标识 Record 类型
offset + 8:  field_0 (f64, 8 bytes)
offset + 16: field_1 (f64, 8 bytes)
...
offset + 8*(n): field_{n-1} (f64, 8 bytes)
```

- 字段按 CalcitStruct.fields 的字母序排列（与 Calcit 语义一致）
- 每个 Record 占用 `8 * (1 + field_count)` 字节

### 操作映射

| Calcit 操作 | WASM 实现 |
|-------------|----------|
| `&%{} struct field1 val1 ...` | bump alloc + f64.store 每个字段 |
| `&record:get record :field` | `f64.load (ptr + field_offset)` |
| `&record:nth record idx tag` | `f64.load (ptr + (1 + idx) * 8)` |
| `&record:assoc record :field val` | 复制整个 Record + 修改指定字段 |
| `:field record` (tag-as-fn) | 等同于 `&record:get` |

### 内存分配

使用 bump allocator（单向增长，不回收）：

```wasm
(global $heap_ptr (mut i32) (i32.const 0))

;; alloc(size: i32) -> i32
(func $alloc (param $size i32) (result i32)
  (local $ptr i32)
  (local.set $ptr (global.get $heap_ptr))
  (global.set $heap_ptr (i32.add (global.get $heap_ptr) (local.get $size)))
  (local.get $ptr)
)
```

- 初始 heap_ptr = 0（或留出少许保留空间）
- 线性内存初始 1 page (64KB)，不够时 `memory.grow`
- 实验阶段不实现 GC，适用于短生命周期的计算任务

## Tuple/Enum 编译

### 内存布局

```
offset + 0:  variant_tag (f64, 8 bytes) — 对应 EnumVariant 的 tag ID
offset + 8:  payload_0 (f64, 8 bytes)
offset + 16: payload_1 (f64, 8 bytes)
...
```

### 操作映射

| Calcit 操作 | WASM 实现 |
|-------------|----------|
| `:: tag val1 val2 ...` | bump alloc + store tag + payloads |
| `&tuple:nth tuple idx` | `f64.load (ptr + (1 + idx) * 8)` |
| `tag-match` | load tag → if/else chain（小量 variants）或 `br_table`（多 variants） |

### tag-match 编译

```
;; tag-match expr
;;   (:ok val) -> body1
;;   (:err msg) -> body2
f64.load ptr  ;; load variant tag
i32.trunc_f64_u
br_table 0 1 2  ;; jump to branch (default to last)
```

对于少量分支（≤4），直接用 if/else 链；多分支用 `br_table`。

## List 支持（暂缓）

List 在 Calcit 中是持久化数据结构（`Vector(Vec)` 或 `TernaryTreeList`），WASM 中实现完整语义工作量大。

### 初步方案（仅 stub）

- `&list:nth` / `count` 可对定长数组实现
- `foldl` / `map` 等高阶操作暂不支持（需要闭包/函数指针）
- 本轮仅添加 `Err("List not yet supported in WASM codegen")`

## String 支持（评估阶段）

### 方案对比

| 方案 | 复杂度 | 适用性 |
|------|--------|--------|
| Host import (JS 侧处理) | 低 | 适合 JS 互操作场景 |
| 线性内存 UTF-8 | 中 | 需要自己维护 string pool |
| WASM GC string (proposal) | 低 | 需要运行时支持 (V8/Deno ✅) |

### 结论

本轮不实现 String。推荐路径：
1. **近期**: host import 方案（`println` 等通过 host function 代理）
2. **远期**: WASM GC string 标准化后直接采用

## 改进路线

### 近期（低投入）

- 输出路径配置（`--emit-path`）
- 跨命名空间函数调用
- `pow` / `sin` / `cos` 通过 host import
- let 嵌套合并优化

### 中期（本文档主题）

- ✅ Tag 编译为整数常量
- Record 线性内存布局 + 基本操作
- Tuple/Enum 线性内存布局 + tag-match
- JS 互操作桥接

### 远期

- List 完整支持（可能需要 WASM GC）
- String 支持
- 完整类型推导
- 多模块链接

## 参考

- 可行性评估: `rfc/04-15-wasm-compilation-feasibility.md`
- 优化目录: `rfc/04-15-type-directed-optimization-catalog.md`
- 用法文档: `docs/wasm-codegen.md`
