# Internal WASM Codegen Validation

## 概述

这是 Calcit 仓库内部的 codegen 验证后端，用于测试编译器、数据布局和运行时假设。它不是公开的 Calcit 编译目标，也不构成用户 CLI 契约。内部 binary 由 `internal-wasm` feature gate 保护，只通过仓库测试脚本调用。

## 支持的子集

| 特性                                   | 支持 | 说明                     |
| -------------------------------------- | ---- | ------------------------ |
| `defn` (固定参数)                      | ✅   | 所有参数和返回值均为 f64 |
| Number 字面量                          | ✅   | 直接映射到 f64           |
| Bool 字面量                            | ✅   | true → 1.0, false → 0.0  |
| Nil                                    | ✅   | → 0.0                    |
| `if` 条件                              | ✅   | 非零为 truthy            |
| `let` 绑定                             | ✅   | 转为 WASM local          |
| 算术: `&+`, `&-`, `&*`, `&/`           | ✅   | 映射到 f64 指令          |
| `&number:rem`                          | ✅   | 通过 trunc/mul/sub 模拟  |
| 比较: `&<`, `&>`, `&=`                 | ✅   | 返回 f64 (1.0/0.0)       |
| `not`                                  | ✅   | 逻辑非                   |
| `identical?`                           | ✅   | 数值相等 (f64.eq)        |
| 数学: `floor`, `ceil`, `round`, `sqrt` | ✅   | 直接映射 WASM 指令       |
| `recur` (尾递归)                       | ✅   | 映射到 WASM loop + br    |
| 函数调用                               | ✅   | 同模块内函数互调         |
| Tag / Record / Tuple                   | ✅   | 线性内存 + f64 编码指针  |
| List / Map / Set                       | ✅   | 线性内存 bump allocator  |
| `println` / `echo` / IO               | ✅   | 通过 `io/log_value` host import |
| 字符串字面量                           | ✅   | 编译期写入数据段         |
| `&str:count` / `&str:first` / `&str:rest` / `&str:slice` | ✅ | UTF-8 字节操作 |
| `&str:nth`                            | ✅   | 返回单字符字符串或 nil |
| `&str:concat`                          | ✅   | bump alloc + `memory.copy` |
| `&str:compare`                         | ✅   | 逐字节字典序比较         |
| `&str:contains?`                       | ✅   | 字节索引范围检查         |
| `&str:find-index`                      | ✅   | 朴素字节子串搜索，返回偏移或 -1 |
| `&str:includes?`                       | ✅   | `find-index >= 0`          |
| `&str:pad-left` / `&str:pad-right`     | ✅   | 循环填充 pattern 字节    |
| `__str_new` (FFI)                      | ✅   | JS → WASM 字符串传递     |
| `defwasm-import` / `defwasm-export`    | ✅   | 显式声明 host ABI，支持 Number / String |

**不支持（留给解释器/JS codegen）：**

- 宏系统（编译前已展开）
- `&str:replace` / `str` 类型转换 / `&str:escape`
- Method dispatch
- Atom / Ref
- 可变参数 (`&`) 和可选参数 (`?`)

## 内部验证方式

```bash
yarn try-wasm
```

脚本会通过 `internal-wasm` feature 构建内部 runner，生成 `js-out/program.wasm`，再使用 Node.js 验证导出函数。不支持的函数会把 skip 信息写到 stderr。

## 声明式 WASM FFI

`defwasm-export` 标记提供给宿主程序的稳定入口；它和 `defn` 使用同一函数形状。若带该标记的定义无法被 WASM codegen 编译，编译会失败，避免把错误的占位函数暴露给宿主：

```cirru.no-check
defwasm-export add (a b)
  &+ a b
```

`defwasm-import` 声明一个由宿主提供的函数。定义体的前两个字符串分别是 WASM import 的 module 和 field：

```cirru.no-check
defwasm-import host-upcase (text)
  |host
  |string-upcase

defwasm-export upcase (text)
  host-upcase text
```

首版 ABI 的所有参数和返回值都是 `f64`。`Number` 直接传递；`String` 传递其 Calcit 字符串的逻辑指针（以 `f64` 表示）。因此宿主应使用下文的字符串布局读取 String，并使用 `__str_new` 或同一布局分配返回 String。`nil` 为 `0`。

为了保持现有内部调试入口兼容，当前模块仍导出可编译的普通 `defn`；`defwasm-export` 的作用是声明并校验面向宿主的 ABI，而不是隐藏旧导出。以后如需要严格的 export allowlist，会以显式编译选项引入。

## 字符串内存布局

字符串在线性内存中以 UTF-8 字节存储，与 Rust 的 `str` 语义一致（`count` 返回**字节数**而非字符数）：

```
logical_ptr - 8: HEAP_MAGIC (i32)        — 堆对象标记
logical_ptr - 4: type_tag_id (i32)       — "string" tag
logical_ptr + 0: byte_len (f64, 8 bytes) — UTF-8 字节数
logical_ptr + 8: UTF-8 bytes             — 填充到 8 字节对齐
```

## JS ↔ WASM 字符串 FFI

WASM 模块导出以下接口供 JS 传递字符串：

- `__heap_ptr`: 可读写的 i32 global，当前堆顶指针
- `__str_new(src_ptr: i32, byte_len: i32) → f64`: 将 `byte_len` 字节从 `src_ptr` 复制到堆中，返回字符串逻辑指针

**零拷贝协议**（JS 向 WASM 传字符串）：

```js
const mem = inst.exports.memory.buffer;
const top = inst.exports.__heap_ptr.value;
const bytes = new TextEncoder().encode("hello");
// 写在 top+16（跳过 8 字节 header + 8 字节 byte_len）
new Uint8Array(mem, top + 16, bytes.length).set(bytes);
// __str_new 在 top+16→top+16 是 memory.copy 无操作，直接写 header
const strPtr = inst.exports.__str_new(top + 16, bytes.length);
```

也可以写到任意地址再传 `src_ptr`，`__str_new` 会执行一次 `memory.copy`。

## 示例

输入（`demos/wasm-demo.cirru`）中的 `fibo` 定义：

```cirru.no-check
defn fibo (n)
  if (&< n 2) 1
    &+ (fibo (&- n 1)) (fibo (&- n 2))
```

编译后输出二进制 `js-out/program.wasm`，可用 `wasm-tools print js-out/program.wasm` 查看反汇编的 WAT 文本。

## 实现位置

- `src/codegen/emit_wasm.rs` — WASM 二进制代码生成（via wasm-encoder）
- `src/codegen.rs` — 模块注册
- `src/cli_args.rs` — `EmitWasmCommand` CLI 定义
- `src/bin/cr_wasm.rs` — feature-gated 内部 runner
- `calcit/test-wasm.cirru` — 测试用例
- `scripts/test-wasm.sh` — WASM 验证脚本（生成 + Node.js 验证，集成在 `yarn check-all` 中）
- `scripts/test-wasm.mjs` — Node.js 测试运行器

## 测试

WASM 验证已集成到 `yarn check-all` 流程中（通过 `yarn try-wasm`）：

```bash
# 直接运行内部验证脚本
bash scripts/test-wasm.sh

# 或通过 yarn
yarn try-wasm
```

## 设计文档

- 设计决策与改进路线见 `RFCs/04-16-wasm-data-structures.md`
- 可行性评估见 `RFCs/04-15-wasm-compilation-feasibility.md`
