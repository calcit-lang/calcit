# WASM Codegen（实验性）

## 概述

Calcit 提供了一个最小化的 WASM 编译目标，将纯数值函数子集编译为二进制 `.wasm` 文件（通过 `wasm-encoder` crate）。这**不是 Calcit 的主打功能**，定位为实验性的“加速岛”（hot island），适用于计算密集的纯函数。

## 支持的子集

| 特性 | 支持 | 说明 |
|------|------|------|
| `defn` (固定参数) | ✅ | 所有参数和返回值均为 f64 |
| Number 字面量 | ✅ | 直接映射到 f64 |
| Bool 字面量 | ✅ | true → 1.0, false → 0.0 |
| Nil | ✅ | → 0.0 |
| `if` 条件 | ✅ | 非零为 truthy |
| `let` 绑定 | ✅ | 转为 WASM local |
| 算术: `&+`, `&-`, `&*`, `&/` | ✅ | 映射到 f64 指令 |
| `&number:rem` | ✅ | 通过 trunc/mul/sub 模拟 |
| 比较: `&<`, `&>`, `&=` | ✅ | 返回 f64 (1.0/0.0) |
| `not` | ✅ | 逻辑非 |
| `identical?` | ✅ | 数值相等 (f64.eq) |
| 数学: `floor`, `ceil`, `round`, `sqrt` | ✅ | 直接映射 WASM 指令 |
| `recur` (尾递归) | ✅ | 映射到 WASM loop + br |
| 函数调用 | ✅ | 同模块内函数互调 |

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

## 设计决策

1. **二进制 `.wasm` 输出** — 使用 `wasm-encoder` crate 直接生成标准 WASM 二进制格式。无需外部工具（如 wasmtime、wat2wasm），Node.js 内置的 `WebAssembly` API 即可加载运行，CI 也无需额外安装依赖。

2. **All-f64 类型策略** — Calcit 只有一种数值类型（f64），WASM codegen 将所有值统一为 f64，Bool 用 1.0/0.0 表示。这避免了类型分析的复杂度。

3. **比较运算的 f64 返回** — WASM 比较指令返回 i32，但我们的值体系是 f64。使用 `select` 指令将 i32 条件转为 f64 (1.0/0.0)。`if` 表达式再用 `f64.ne x 0.0` 转回 i32 条件。

4. **recur 映射到 WASM loop** — `recur` 在 Calcit 中用于尾递归。WASM codegen 检测函数体是否包含 recur，如果是则整个函数体包装在 `(loop (result f64) ...)` 中，`recur` 编译为临时变量赋值 + `br` 到 loop 标签（通过 block_depth 追踪嵌套深度）。

## 实现位置

- `src/codegen/emit_wasm.rs` — WASM 二进制代码生成（via wasm-encoder）
- `src/codegen.rs` — 模块注册
- `src/cli_args.rs` — `EmitWasmCommand` CLI 定义
- `src/bin/cr.rs` — `run_wasm_codegen` 入口
- `calcit/test-wasm.cirru` — 测试用例（纯数值函数集）
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

测试覆盖的函数：fibo, factorial, add-two, sum-range, floor, ceil, round, sqrt, rem, compare, not, let-chain, collatz-steps, gcd。

## 未来改进路线

### 近期（低投入）

- **输出路径配置** — 目前固定输出到 `js-out/program.wasm`，应支持 `--emit-path` 指定
- **跨命名空间函数调用** — 当前仅编译 init 命名空间中的函数，应支持跨 ns 调用
- **let 嵌套优化** — 预处理后的 `CoreLet` 链已做扁平化，但可进一步合并连续 local.set
- ~~**更多数学函数**~~ — ✅ 已实现 `floor`, `ceil`, `round`, `sqrt`（直接映射 WASM 指令）
- **`pow` 支持** — WASM 无 `f64.pow` 指令，需通过 host import 或整数幂展开实现
- **`sin`/`cos` 支持** — WASM 无三角函数指令，需通过 host import 实现

### 中期（需要较多工作）

- **JS 互操作桥接** — 生成 JS wrapper，允许 JS codegen 的代码调用 WASM 编译的热函数
- **Record 支持** — 将 Record 映射到线性内存 struct（字段按排序索引直接偏移），需要完整的类型信息
- **Tag/Enum 支持** — Tag 编译为整数常量，tag-match 编译为 `br_table` 跳转表

### 远期（需要基础设施变更）

- **WASM GC** — 利用 WASM GC proposal（V8/SpiderMonkey 已支持）映射 persistent data structure
- **String 支持** — 通过 host function import 或 WASM GC 的 string 类型
- **完整类型推导** — AOT 编译需要全量类型标注，依赖类型系统覆盖率提高
- **多模块链接** — WASM module linking proposal，支持分模块编译

### 参考

详细的可行性评估见 `drafts/04-15-wasm-compilation-feasibility.md`，包括：
- 三条路径分析（解释器→WASM、AOT→WASM、WASM GC）
- 依赖兼容性审计
- 全局状态审计
- 内存模型设计
