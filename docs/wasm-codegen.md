# WASM Codegen（实验性）

## 概述

Calcit 提供了一个最小化的 WASM 编译目标，将纯数值函数子集编译为二进制 `.wasm` 文件（通过 `wasm-encoder` crate）。这**不是 Calcit 的主打功能**，定位为实验性的“加速岛”（hot island），适用于计算密集的纯函数。

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

**不支持（留给解释器/JS codegen）：**

- 字符串操作
- 宏系统（编译前已展开）
- Record / Map / Set / List
- Method dispatch
- Atom / Ref
- FFI / println / IO
- 可变参数 (`&`) 和可选参数 (`?`)

## 使用方式

```bash
# 编译为 .wasm 二进制
cr demos/wasm-demo.cirru wasm

# 输出在 js-out/program.wasm
# 不支持的函数会打印 skip 信息到 stderr

# 用 Node.js 加载和运行
node -e "
const fs = require('fs');
const wasm = fs.readFileSync('js-out/program.wasm');
const mod = new WebAssembly.Module(wasm);
const inst = new WebAssembly.Instance(mod);
console.log(inst.exports.fibo(10));      // 89
console.log(inst.exports.factorial(10)); // 3628800
"
```

## 示例

输入（`demos/wasm-demo.cirru`）中的 `fibo` 定义：

```cirru
defn fibo (n)
  if (&< n 2) 1
    &+ (fibo (&- n 1)) (fibo (&- n 2))
```

编译后输出二进制 `js-out/program.wasm`，可用 `wasm-tools print js-out/program.wasm` 查看反汇编的 WAT 文本。

## 实现位置

- `src/codegen/emit_wasm.rs` — WASM 二进制代码生成（via wasm-encoder）
- `src/codegen.rs` — 模块注册
- `src/cli_args.rs` — `EmitWasmCommand` CLI 定义
- `src/bin/cr.rs` — `run_wasm_codegen` 入口
- `calcit/test-wasm.cirru` — 测试用例
- `scripts/test-wasm.sh` — WASM 验证脚本（生成 + Node.js 验证，集成在 `yarn check-all` 中）
- `scripts/test-wasm.mjs` — Node.js 测试运行器

## 测试

WASM 验证已集成到 `yarn check-all` 流程中（通过 `yarn try-wasm`）：

```bash
# 单独运行 WASM 测试
bash scripts/test-wasm.sh

# 或通过 yarn
yarn try-wasm
```

## 设计文档

- 设计决策与改进路线见 `rfc/04-16-wasm-data-structures.md`
- 可行性评估见 `rfc/04-15-wasm-compilation-feasibility.md`
