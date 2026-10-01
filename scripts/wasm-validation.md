# WASM 编译与验证

## 概述

Calcit 通过两个公开 preview 子命令暴露 WASM codegen：`calcit wasm` 生成 browser/embedded core module，`calcit wasi` 默认生成 WASI 0.3.1 `wasi:cli/command` Component；只有显式 `--boundary native` 才选择 Preview 1 command core module。两者共享同一套 Snapshot 加载、预处理、target validation 与 codegen 实现。

用户和 AI Agents 优先查已安装版本的 `calcit wasm --help`、`calcit wasi --help` 及[CLI 参考](../docs/run/cli-options.md#wasm-preview-命令)。本页解释仓库回归、内部表示与已经验证的子集，不另建用户命令或把 backend 缺口改写成语言禁令。

`cr-wasm` 已退出默认安装和 release assets。新的人类与 Agent 工作流应使用 `calcit wasm` / `calcit wasi`，不要依据内部 binary 名称猜测输出契约。仓库内的 WASI 自举回归使用 feature-gated harness；它不是用户 CLI，也不会由默认 `cargo install calcit` 安装。

## 边界分层

WASM 相关能力分属四个不同层级，排查或文档引用时不要把它们混在同一张表里：

1. **表层与 typed core**：Calcit macro 在编译前展开。普通方法调用可由类型推导、trait 证明和
   单态化 lowering 成静态调用；应优先保留这些 Calcit 写法，不为绕过后端改成 native call。
   尚未取得闭合类型、依赖运行时动态派发或需要未实现 ABI 的调用可能被 WASM 拒绝，
   这是相应目标的支持边界，不是宏、方法、Dynamic、Ref 等在整个 Calcit 语言中非法。
2. **预处理后可静态 lowering 的子集**：下表列出的纯计算与闭合数据操作，在 core 与 WASI command
   两个目标上共享同一套 Snapshot 加载、target validation 与 codegen。
3. **WASI command 目标**：默认 WASI 0.3.1 Component 使用标准 command world；显式
   `--boundary native` 生成带 `_start` 的 Preview 1 command core module。宿主能力按所选边界
   验证，不静默退回旧协议，也不继承 browser core 目标的 JS `io` imports。
4. **Component boundary**：`calcit wasm --boundary component` 与
   `calcit ffi export --boundary component` 走独立的 Canonical ABI adapter 与版本化 Interface IR，
   其类型矩阵、async lifecycle 与 HTTP 边界以
   [WASM Component 边界](../docs/installation/wasm-component-boundary.md) 为准，不在本文件重复。

下表主要描述第 2 层的内部表示和部分已验证操作，不保证某个类型的所有方法或动态输入均可编译；
WASI 宿主能力必须另外满足所选 command 边界，Component 类型闭包也必须单独验证。

## 支持的子集

| 特性                                   | 支持 | 说明                     |
| -------------------------------------- | ---- | ------------------------ |
| `defn` (固定参数)                      | ✅   | 所有参数和返回值均为 f64 |
| Number 字面量                          | ✅   | 直接映射到 f64           |
| Bool 字面量                            | ✅   | true → 1.0, false → 0.0  |
| Nil                                    | ✅   | → 0.0                    |
| `if` 条件                              | ✅   | Bool 的内部 0/1 分支；不能据此把任意 Number 条件当作合法表层写法 |
| `let` 绑定                             | ✅   | 转为 WASM local          |
| 算术: `&+`, `&-`, `&*`, `&/`           | ✅   | 映射到 f64 指令          |
| `&number:rem`                          | ✅   | 通过 trunc/mul/sub 模拟  |
| `&number:fits?`                        | ✅   | 支持字面量或局部绑定 tag；未知 refinement tag 返回 false，不 trap |
| `to-string`（Number）                  | ✅   | 首选公开转换；运行时按 native/JS 共用的最短十进制规则输出，特殊值、有限小数和指数边界均覆盖；旧 turn-string 暂留兼容 |
| 比较: `&<`, `&>`, `&=`                 | ✅   | 返回 f64 (1.0/0.0)       |
| `not`                                  | ✅   | 逻辑非                   |
| `identical?`                           | ✅   | 数值相等 (f64.eq)        |
| 数学: `floor`, `ceil`, `round`, `sqrt` | ✅   | 直接映射 WASM 指令       |
| `recur` (尾递归)                       | ✅   | 映射到 WASM loop + br    |
| 函数调用                               | ✅   | 同模块内函数互调         |
| Tag / Struct / Enum                    | ✅   | 线性内存 + f64 编码指针  |
| List / Map / Set                       | ✅   | 线性内存 bump allocator  |
| `println` / `echo`                     | ✅   | browser core 使用 `io` host imports；WASI command 使用对应标准流协议，不保证所有 IO 均受支持 |
| 字符串字面量                           | ✅   | 编译期写入数据段         |
| `&str:count`                          | ✅   | Unicode 标量数；不是 UTF-8 字节数 |
| `&str:first` / `&str:rest` / `&str:slice` / `&str:nth` | ✅ | 按 Unicode 标量读取/切片；空串与越界安全，非法索引 trap；共享 Calcit 测试见 `scripts/check-string-unicode.mjs` |
| `&str:concat`                          | ✅   | bump alloc + `memory.copy` |
| `&str:compare`                         | ✅   | 逐字节字典序比较         |
| `&str:contains?`                       | ✅   | Unicode 标量索引范围检查；负数/小数/非有限值 trap |
| `&str:find-index`                      | ✅   | 子串搜索返回 Unicode 标量索引或 -1；与 `.get/.slice/.len` 使用相同单位，共享 Calcit 测试覆盖中文、emoji、组合字符与求值顺序 |
| `&str:includes?`                       | ✅   | `find-index >= 0`          |
| `&str:pad-left` / `&str:pad-right`     | ✅   | 循环填充 pattern 字节    |
| `format-cirru-edn`                     | 部分 | 由闭合静态类型导向；支持标量、递归容器、Struct field 与 Enum payload |
| `try-parse-cirru-edn-as`               | 部分 | 复用闭合 `DataShapeGraph`；支持递归容器与 Struct/Enum/Option/Result nominal value |
| `__str_new` (FFI)                      | ✅   | JS → WASM 字符串传递     |
| `defwasm-import` / `defwasm-export`    | ✅   | 显式声明 host ABI，支持 Number / String |

**需要明确检查的目标边界：**

- `&str:replace` / `str` 类型转换 / `&str:escape`
- 需要运行时选择实现的动态 method dispatch；已证明的普通方法不在此列
- Atom / Ref 的运行时状态与观察者能力
- 未经静态 lowering 的可变参数 (`&`) 和可选参数 (`?`)；公开 Component ABI 不接受这些 arity

上述缺口不能通过放宽 Dynamic、复制一套“动态 API”或自动插入 unsafe 来隐藏。普通依赖中
未受支持的实现可能保留 trapping slot；显式导出与 command 入口则要求更强的编译期验证。
生成产物存在不是可用性证明，应实际调用相应入口并验证结果。

### Cirru EDN 格式化边界

WASM 的 Number、Bool、nil 和 tag id 当前共用 f64 value ABI，运行时反射无法可靠区分这些标量。因此 `format-cirru-edn` 不增加动态类型探测规则，而是读取预处理后保留的闭合类型并生成对应 formatter。

当前支持 nil、Bool、String、tag、整数 numeric refinement、直接 Number 字面量，以及闭合的递归 `List<T>`、`Map<K,V>`、Struct 与 Enum；`Option<T>`、`Result<T,E>` 作为 core nominal Enum 复用同一路径。嵌套容器和 nominal payload 使用 Cirru EDN 的括号表达式布局，仍由编译期类型决定，而不是回退为 Dynamic。直接 Number 字面量在 codegen 时复用 native formatter，保留精确字节语义。Map key 仍限定为可按 native 规则稳定排序的标量，value 可以是闭合递归值。输出与 native compact formatter 一样带首尾换行，字符串遵循 Cirru leaf 的 `|text` / `"|quoted text"` 规则。WASM formatter 的单次输出分配上限为 64 KiB，超过上限会在分配前 trap；回归脚本还会把真实 Wasmtime 输出逐值交给 `cirru_edn` 重新解析。`Dynamic`、未解析类型变量与运行时普通 `Number` 仍以稳定诊断明确拒绝，不能静默猜测或输出占位数据。

`try-parse-cirru-edn-as` 的 WASM 实现直接消费预处理阶段生成的同一份闭合 `DataShapeGraph`，不先解析成 `Dynamic`，也不引入第二套 decoder API。当前支持 nil、Bool、Number、整数/浮点 refinement、bare 或 quoted/escaped String、编译产物已知 tag，以及递归闭合的 `List<T>`、`Map<K,V>`、Struct 与 Enum；Enum 的类型名、variant、payload 数量和每项 payload shape 都在生成的专用 parser 中验证，Option/Result 不走旁路。嵌套格式化器输出的括号表达式会在进入子节点时解开，再交给对应的静态 shape parser。标量接受 bare token 和 formatter 产生的顶层 `do token`。quoted String 只接受 formatter 使用的 `\n`、`\t`、`\"`、`\\` 四种 escape，未知或截断 escape 返回语法错误。输入上限为 64 KiB，List 另设 4096 item 上限，Map 另设 2048 entry 上限；超限、语法错误、numeric refinement 越界、未知 tag 或 nominal Enum 不匹配都返回稳定的 `Result :err`，不触发 trap。Map 的运行时 String key 按 UTF-8 内容 hash 和比较，因此 parser 新建的 key 可由等值字符串稳定查询。

这项能力复用现有 core API，不增加新的 CLI 或 Calcit 表层入口。WASI 文件工作流仍通过 `fs:path` 的 typed read/write API 组合。

仓库回归还把这两项能力组合成一条真实的预开放文件流程：从 `/workspace/input.cirru` 读取
`Map<String, Int32>`，在 Calcit 代码中更新数据，再把规范化的 Cirru EDN 写入
`/workspace/output.cirru`。输入类型错误或数值 refinement 越界会沿 `Result` 分支以状态码 `1`
退出，不产生输出文件，也不触发 WASM trap。这个文件闭环仍以顶层标量 Map 保持最小示例；递归容器和
Struct、Enum、Option 与 Result 已由同一套 parser/formatter 覆盖，不会增加新的命令入口。

`examples/wasi-command/calcit.cirru` 的具名 `Manifest` 业务入口另由
`scripts/test-wasi-manifest-component.sh` 在 Wasmtime 49/WASI 0.3.1 上验证；它与
`scripts/test-wasi-manifest-business.sh` 的 native、Node JS、Preview 1 使用同一份输入、期望输出和
`Result` 分支。`calcit wasi` 默认生成 WASI 0.3 Component；Preview 1 需显式 `--boundary native`。Component 运行需要 `-S p3`、两个 Component async
开关及 `--dir HOST::/workspace`。写入是 create + truncate，失败可能留下截断或部分文件，
不能误称为原子替换。
同一 Snapshot 的 `method-eval-main!` 单独验证普通 `Result.map`：有副作用的 receiver 和回调实参
在 native、Node JS、真实 Component 均按 `receiver`、`argument`、`api!` 各输出一次，不修改文件业务结果。

## 编译与验证方式

生成 browser/embedded core module：

```bash
calcit wasm calcit.cirru --emit-path js-out
```

生成并运行默认 WASI 0.3.1 command Component（下列标准流参数与 CI 使用的 Wasmtime 49.0.1 对齐）：

```bash
calcit wasi tests/fixtures/wasi-command-03.cirru --emit-path target/wasi-doc-component
wasmtime run -S p3 -W component-model-more-async-builtins=y -W component-model-async-stackful=y target/wasi-doc-component/program.wasm
```

如果确实需要尚未迁移到 Component 的宿主能力，显式选择 Preview 1，不把它当作默认推荐：

```bash
calcit wasi calcit/test-wasi-command.cirru --boundary native --emit-path target/wasi-doc-native
wasmtime run --env CALCIT_WASI_TEST_ENV=release target/wasi-doc-native/program.wasm alpha beta
```

两个仓库 fixture 的入口契约不同：Component 示例显式返回 Unit；Preview 1 的历史测试入口
仍返回 Number，因此不能把它换掉参数直接作为默认 Component 示例，否则会报告
`E_WASI_COMMAND_ENTRY`，要求入口返回 Unit。业务入口以现有
`examples/wasi-command/calcit.cirru` 与相应回归脚本为准，不额外创造一个示例程序。

两个命令都支持 `--check-only`。WASI 0.3 Component 还执行 codegen 与封装验证，但不写出
`program.wasm`；它不是“只有预处理”。`--help` 会分别说明输出类型和 command entry 约束。

仓库全量验证：

```bash
yarn try-wasm
```

脚本通过公开 `calcit wasm` 命令生成 `js-out/program.wasm`，再使用 Node.js 验证导出函数。不支持的函数会把 skip 信息写到 stderr。

## Core 与 WASI command 目标

公开命令名称明确区分两种宿主契约：

- `calcit wasm` 默认 core boundary，供浏览器或嵌入式宿主提供已有的 `math`、`io` imports。通用 `--boundary component` 是供工具包装的 core adapter，不是默认 WASI command。
- `calcit wasi` 默认 WASI 0.3.1 command Component；init 必须零参数并显式返回 Unit。支持标准流、参数/环境、`quit!`、`read-stdin-text` 以及 `FsPath .read-text` / `.write-text!`，不等同于所有宿主能力均可用。
- `calcit wasi --boundary native` 才是带 `_start` 的 Preview 1 command。时钟、等待、安全随机数和 `.read-dir` 等尚未迁移的能力仍需此显式边界，不能在默认 Component 路径静默回退。

command init definition 正常返回时状态为 `0`；`quit!` 接受 `0..255` 的整数，映射到所选协议的退出操作（Preview 1 为 `proc_exit`）。该限制与原生、JavaScript 后端一致，避免宿主各自执行隐式饱和或取模。

WASI 目标不会接受 `defwasm-import` 声明的任意宿主函数，也不会继承 core 目标的 JS `io` imports。通用目标校验使用 `E_WASM_CAPABILITY` / `E_WASM_TARGET`；WASI 0.3 command 另明确拒绝尚未迁移的能力（`E_WASI_COMMAND_CAPABILITY`）、无法证明的间接调用（`E_WASI_COMMAND_INDIRECT`）及通用 export（`E_WASI_COMMAND_EXPORT`）。这些诊断是目标证据，不是增加一套表层语言规则；Preview 1 的 ABI 名称也不会成为 Calcit 源码 API。

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

普通 `defn` 只在模块内部可调用，不再自动成为宿主 ABI。公开 WASM export surface 只包含显式 `defwasm-export` 声明，以及有保留名称的 runtime symbols（`memory`、`__heap_ptr`、`__str_new`、`__string_tag`、WASI 的 `_start` 与 Component 的 `cabi_*` / adapter symbols）。因此面向宿主的函数必须显式声明，缺失的声明不会再被历史调试兼容隐式导出：

```cirru.no-check
defn internal-helper (x)
  &+ x 1

defwasm-export add-one (a)
  internal-helper a
```

内部函数如果无法编译，仍保留一个 trapping slot 以维持 call/table 索引稳定，但该 slot 不进入公开 export surface；显式 `defwasm-export` 无法编译时，codegen 直接失败而不是暴露占位函数。迁移旧项目时，把宿主会调用的 `defn` 改成 `defwasm-export` 即可；不需要新增命令入口。

## 字符串内存布局

字符串在线性内存中以 UTF-8 字节存储，下面的 `byte_len` 是存储字节数。公开 `.len` 与底层 `&str:count` 返回 **Unicode 标量数**，不等于该内存字段，也不表示用户感知的字形簇数；相关跨 backend 语义由 `scripts/check-string-unicode.mjs` 验证：

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
- `src/wasm_cli.rs` — 两个公开命令与内部回归 harness 共用的加载和编译流程
- `src/bin/wasi_preprocess_harness.rs` — 仅在 `internal-wasi-preprocess-harness` feature 下构建的内部回归 harness
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

## 手动渐进套件（不在 CI 中）

`scripts/test-wasm.sh` 是通用 export/ABI 验证入口；CI 与 `yarn check-all` 还运行 String、数字谓词等共享 Calcit 契约脚本，具体入口以 `.github/workflows/test.yaml` 和 `package.json` 为准。数字契约脚本的十个 refinement 转换测试完整运行在 native/JS，WASM/WASI 使用原有四个整数谓词测试；两组覆盖不能混称。关键 API 的测试证据索引见 [核心 API 契约基线](../docs/data/README.md#已验证的后端范围)。以下脚本是维护者按需运行的渐进式 / 调试工具，
不属于默认门禁；在此登记用途，避免它们因无入口而悄悄腐化：

- `scripts/test-wasm-suite.sh`：逐个把纯计算 `test-*.cirru` fixture 编译为 WASM 并运行 `main!`。
- `scripts/test-wasm-suite-extended.sh`：编译 `calcit/test-wasm-suite.cirru` 多模块入口，在一个 WASM
  实例中顺序运行各模块 `main!`，目标随 WASM 支持范围逐步扩大。
- `scripts/test-wasm-run.mjs`：上面的通用 WASM runner（读取 `js-out/program.wasm`，调用 `main!`）。
- `scripts/test-wasm-call.mjs`：手动调用指定 WASM export 的调试助手。

```bash
# 默认门禁
bash scripts/test-wasm.sh

# 手动渐进套件（用 CALCIT_BIN 指定已构建的二进制）
CALCIT_BIN=./target/debug/calcit bash scripts/test-wasm-suite.sh
CALCIT_BIN=./target/debug/calcit bash scripts/test-wasm-suite-extended.sh
node scripts/test-wasm-call.mjs <export-name>
```

## 设计文档

- 设计决策与改进路线见 `RFCs/04-16-wasm-data-structures.md`
- 可行性评估见 `RFCs/04-15-wasm-compilation-feasibility.md`
