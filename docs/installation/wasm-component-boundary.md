---
title: "WASM Component 边界"
summary: "区分 Calcit core-module value ABI、Component contract 与 Canonical ABI adapter 的职责"
scope: "core"
kind: "spec"
category: "installation"
aliases:
  - "component model"
  - "canonical abi"
  - "defwasm export"
entry_for:
  - "calcit ffi export --boundary component"
id: core/wasm/component-boundary
parent: core/wasm
related:
  - core/ffi/interface-ir
  - core/installation/host-capability-boundary
---

# WASM Component 边界

Calcit 将 WebAssembly core module 的内部 value ABI 与 Component Model 的公开类型边界分开。这两层不能通过改名或只生成 WIT 互相替代。

## 当前 core-module ABI

默认 native boundary 下，`defwasm-import` 和 `defwasm-export` 使用 Calcit value ABI：

- 参数和返回值统一用 WASM `f64` 承载；
- `Number` 直接传递，`String`、`List`、Struct 和 Enum 传递 Calcit heap 中的逻辑指针；
- 宿主需要理解 Calcit 的 tag、heap 和 memory layout；
- 可编译的普通 `defn` 仍作为内部调试入口导出，`defwasm-export` 则标记必须稳定编译的宿主边界。

这一 ABI 适合直接控制 memory 的 JavaScript 宿主和回归测试，但不是 Canonical ABI。

## 目标分层

Component 路径分为三层：

1. Calcit 类型推导产生闭合的 function schema。`defwasm-import` / `defwasm-export` 只补充 direction 和 symbol，不建立第二套类型规则。
2. core 导出版本化 Component contract，并生成 Canonical ABI 与 Calcit value ABI 之间的 lift/lower adapter。
3. `calcit-bindgen` 从 contract 生成 WIT，负责 component packaging、manifest、ABI fingerprint、compatibility diff 和 runtime integration。

core 不带 WIT generator、WIT golden、component packaging 或 stale-artifact policy；`calcit-bindgen` 不猜测 Calcit heap layout，也不重做类型推导。

## 同步 Component 边界

0.14.20 已为 `Unit` 结果、`Bool`、`Buffer`、`Number`、UTF-8 `String`、递归同质
`List<T>`，以及闭合单态的 `Option<T>` / `Result<T,E>` 生成同步 Canonical ABI adapter。
0.15.1 进一步覆盖 monomorphic Struct record 与闭合单态 Enum；0.15.2 加入明确宽度数值类型。

Calcit 包本身只生成供 packaging 消费的 core module，不在主仓库内生成 WIT 或包装 Component。
独立的 `calcit-bindgen` 已能为基础类型、Option/Result、Struct 与 Enum 生成 WIT、包装 runnable Component，
并通过 Wasmtime 与 jco/Node 验证。
项目仍需显式安装和调用该工具，不能把裸 core module 当作 Component 产物。

Bool 的 Canonical ABI 使用 `i32`
的 `0`/`1`；import 与 export 两个方向都会验证输入和返回值，遇到其他整数、小数、负数、
NaN 或越界数值时直接 trap，不把非规范值静默解释为真假。Buffer 映射为 WIT
`list<u8>`，按原始字节的 `(ptr,len)` 传输；空 Buffer、内嵌零和非 UTF-8 字节都必须原样保留，
越界范围直接 trap。`List<T>` 直接复用类型推导得到的 item type，不增加运行时猜测；当前可递归组合
`Bool`、`Buffer`、`Number`、明确宽度数值、UTF-8 `String` 与 `List<T>`，例如 `List<List<Number>>`。列表的
Canonical ABI 使用元素区间 `(ptr,len)`，并按元素类型递归 lift/lower；空列表、嵌套空列表、非法 Bool
元素、长度乘法溢出和越界区间都按同一套规则处理。

`Option` 与 `Result` 直接使用类型推导保留的 `calcit.core` 名义类型，不会把旧 `Optional<T>`、用户自定义同名 Enum
或 Dynamic 值猜成这两个专用类型。它们与普通 Enum 共用 Canonical ABI variant layout：discriminant 后按 payload 的最大对齐
放置 payload，flat shape 则按 Canonical ABI join 规则合并各分支。当前覆盖 `Option<Number>`、`Option<String>`、
`Result<Number,String>`、`Result<Unit,String>` 与 `Result<List<Number>,String>`，import/export 两个方向都会验证
discriminant、Calcit 内部 enum tag/count、Bool 值和递归 memory range；非法输入直接 trap。`Unit` 结果对应零个
Canonical ABI result，Unit 参数应直接省略而不是占据一个伪值位置。Calcit 内部 value ABI 均由 adapter 隔离。

Calcit 已提供 `'Int8`、`'UInt8`、`'Int16`、`'UInt16`、`'Int32`、`'UInt32`、`'Int64`、`'UInt64`、
`'Float32` 与 `'Float64` 作为 source-level 数值 refinement，并通过 `number->int8` 等函数显式建立
受检边界证明。它们的运行时表示仍是统一的 `Number`。Component Interface IR v3 已将这些类型
分别导出为独立 `kind`；adapter 不能依据值范围或调用位置猜测宽度，也不能静默退化为 `Number`。
adapter 直接按 Canonical ABI 使用 `i32`、`i64`、`f32` 和 `f64` flat shape 及对应 memory layout。
两个方向都会拒绝越界、非整数、符号错误、无法精确进入 `f64` 的 64 位整数，以及不能原样往返的 `Float32`，
不引入兼容 writer 或运行时类型猜测。

## 类型闭包

0.15.1 已加入 monomorphic Struct record 的同步 core adapter：字段名称、顺序与嵌套类型来自规范化后的
`defstruct` schema，record field layout 复用与 List/Option/Result 相同的递归 value walker；不会从运行时值猜测字段。
Struct 参数按字段 flat shape 展开，多结果返回仍使用 Canonical ABI return area；lift/lower 都验证 canonical memory、
Calcit 内部 field count 与 nominal struct tag。嵌套 Struct 以及字段中已经受支持的闭合类型共用同一套递归规则，
generic 必须在边界处已经具体化。同步 direct adapter 会在 schema 阶段拒绝总计超过 16 个 flat values 的参数签名，
在 indirect parameter lowering 完成前不会生成不符合 Canonical ABI 的 core 函数。

闭合单态 Enum 的 case 顺序来自规范化后的 `defenum` schema，并直接决定从零开始的 discriminant；无 payload、
单 payload 和多 payload case 都使用同一套递归布局，多 payload 在 WIT 中由 consumer 表达为 tuple。payload 可以继续包含
已支持的闭合类型与 Struct。边界不会根据运行时 tag 猜测 declaration，也不会接受 anonymous/open Enum、generic Enum、
递归 Enum 或无法解析的声明；无 payload case 应直接省略 payload，显式 `Unit` payload 会给出带 schema path 的错误。

明确宽度的整数/浮点数已在 0.15.2 具备 source-level refinement，Component Interface IR v3
会确定性导出对应宽度，Canonical ABI adapter 复用相同递归 walker 处理 Struct、Enum、List、Option 与 Result 中的嵌套值。

以下形状不进入同步 Component 边界，并在 contract 导出或 adapter 生成阶段拒绝：

- `Dynamic`、Fn/closure 和 `Ref`；
- JavaScript object 与 opaque host object；
- 没有显式传输表示的 Map/Set；
- 没有 monomorphize 的 generic；
- rest/optional arity 或不完整 schema。

诊断必须包含 definition 和 schema path，不能退化为 universal value，也不能把错误推迟到 runtime。

## CLI 收敛

Component contract 沿用 `calcit ffi export`，通过 `--boundary component`
显式选择。该边界不新增顶层 component/WIT 命令。

- Component Interface IR v3 默认输出 Cirru EDN；
- `--format json` 用于需要 JSON 的 consumer；
- native raw-binding inventory 保留 human 输出和显式 `--json` 入口，但 consumer 必须检查新的 schema version；
- Cirru EDN 和 JSON 必须表达同一个 versioned contract 与 revision。

生成 adapter 时继续沿用 `calcit wasm`：

```bash
calcit wasm calcit.cirru --boundary component --emit-path target/component-core
```

当前 adapter 覆盖 `Unit` 结果、`Bool`、`Buffer`、`Number`、明确宽度数值、UTF-8 `String`、递归同质
`List<T>`、闭合单态 Option/Result、monomorphic Struct record 与闭合单态 Enum 的同步 import/export。
Bool 使用 Canonical ABI `i32` 并严格限制为 `0`/`1`；Number 直接使用 `f64`；明确宽度数值使用
Canonical ABI 对应的 `i32`、`i64`、`f32` 或 `f64`。Buffer 映射到 WIT
`list<u8>`，String 映射到 WIT `string`；`List<T>` 只接受闭合、单态且 item type 已受支持的 schema，
并递归使用对应元素 layout，不把异构值或 `Dynamic` 猜成列表元素。Buffer、String 和 List 的 core 参数
都展开为 `(ptr,len)`，但 lift 后保持不同的 Calcit 类型。Option/Result 与普通 Enum 使用统一 variant layout
与 flat join；Struct 按规范化 schema field layout 递归组合相同 value shape，不为每个类型组合增加独立规则。
export 对应 `canon lift`，超过同步 Canonical ABI 单结果上限的 flat results 返回指向 return area 的 `i32`
指针；import 对应 `canon lower`，结果使用 caller 传入的 return area，再复制回对应的 Calcit 值。两者是
Canonical ABI 针对不同方向规定的函数形状，不是可以互换的自定义约定。core module 只保留显式声明的
Component imports，同时导出 `memory` 与可按需增长 memory 的 `cabi_realloc`，不会携带 native core target
的隐式 `math/io` imports。post-return、stackless callback cancellation 与 stream 仍是后续任务；尚未 lowering 的 schema
继续在生成阶段明确失败。

`cabi_realloc` 与 Calcit 内部对象当前共享同一个 bump pointer，但两类 allocation 的对齐语义不同：canonical byte range
只保证调用方请求的 alignment，Calcit 内部 Struct、Enum、List 等 f64-backed object 则在写入 header 前恢复 8 字节对齐。
因此 String/Buffer 的奇数长度 allocation 不得让后续 nominal object 的 pointer 依赖调用顺序；adapter 仍会把未对齐的
外部 canonical pointer 当作非法输入 trap。

## 异步 Component 合约

0.15.3 从函数 schema 已有的 `:async true` 推导 Component definition 的必填
`invocation: async`；没有该标记的边界确定性导出为 `invocation: sync`。该字段只声明调用语义，
不会把 native `async-task-v1` 的句柄、事件队列或 scheduler 规则搬进 Component contract。

异步函数的参数与返回 schema 和同步函数使用同一套闭合类型规则，typed `Result<T,E>` 仍作为
普通返回类型保留。WASI 0.3 adapter 负责 async function 的 Canonical ABI、取消与 drop；
Calcit 表层不重复引入 `Task<T>`。`stream<T>` 需要先明确单一 readable ownership、背压、取消和
drop，暂不由 v3 contract 猜测或生成。

当前 core adapter 已覆盖 async export，以及 direct/indirect async import。导出的 core 函数沿用参数的 Canonical ABI flat shape，
但没有 core result；Calcit 返回值会按返回 schema lower，并且恰好调用一次 packaging 注入的 `task.return`。
stackful core export 使用 `[async-lift-stackful]<export-symbol>`，`task.return` import 使用模块名
`[export]$root` 与字段名 `[task-return]<export-symbol>`；`wit-component` 据此将每个字段连接到具有对应
result type 的 `canon task.return`。这一命名只存在于 core 与 packaging 的内部契约，不是新的 Calcit API。

显式 async import 使用 `[async-lower]<import-symbol>`；在不超过 4 个 flat 参数时直接传参，更宽的签名按 Canonical ABI 把全部逻辑参数写入一个
对齐后的 parameter record，并向 raw async import 传递单个 record pointer。两种形状都始终通过 return area 写入
非 Unit 结果。adapter 解码 async lower 返回值低 4 位：`0`/`1` 保留输入与输出内存，将高位 subtask 加入独立
waitable set，并等待 `starting`、`started` 到 `returned`；`2` 表示无需 subtask 的立即返回。收到终态后先从 set
移除再分别 drop subtask 与 set，typed `Result<T,E>` 继续按普通返回 schema lift；不会临时退回同步 import ABI。

packaging 还需连接 `$root` 下的 `[waitable-set-new]`、`[waitable-set-wait]`、`[waitable-set-drop]`、
`[waitable-join]` 与 `[subtask-drop]` canonical builtins。当前 stackful adapter 会清理并 trap 已收到的
`cancelled-before-started` / `cancelled-before-returned` 终态；主动取消与向 Calcit 传递 cancellation 需要 stackless
callback 或新的 typed cancellation 语义，不伪装成普通 typed error。完整 runnable Component 继续由 calcit-bindgen 验收。

## 有界 WASI HTTP client

0.16.1 首先提供 client-only、完整缓冲的 HTTP 能力，不等待 streaming、service 或完整 cancellation。
Calcit contract 使用 `calcit:wasi-http/client.request`，request 和 response 都是闭合的 Struct/Enum，
body 只允许 empty、UTF-8 text 或 bytes；每次请求必须携带 `max-response-bytes`。网络未授权、宿主不支持、
请求非法、传输失败和响应超限分别返回 `HttpError`，不得伪造空的成功响应。

可重复的构建链仍然只使用已有入口，contract 默认输出 Cirru EDN：

```bash
calcit app.cirru wasm --boundary component --emit-path target/component-core
calcit app.cirru ffi export --boundary component > target/component-interface.cirru
calcit-bindgen generate target/component-interface.cirru \
  --core-module target/component-core/program.wasm \
  --out target/component
```

宿主使用 `calcit-bindgen 0.1.2` 的 `wasmtime-http` feature，把生成目录中的
`rust/wasmtime_http_adapter.rs` include 到 crate 根部，再调用 `add_to_linker`。默认配置拒绝全部网络；
必须用 `WasiHttpConfig::allow_origin("scheme://authority")` 精确授予 origin，端口属于 authority。
生成器会保留最终 Component 的 `calcit:wasi-http/client` import identity，不要求业务改用内部 WIT alias。

CI 的可用性基线不是“能生成 WIT”：同一份 Calcit contract 必须分别经过 JS host 和 Wasmtime host，
请求真实本机 HTTP 服务，并覆盖成功、capability denied 与 response-too-large。用户可观察的 Result、Struct
和 Enum 形状由定义上的 `:tests` 固定；Rust/JS 测试只验证 packaging、Canonical ABI 和真实宿主行为。
当前明确不覆盖 streaming body、HTTP service、redirect 自动跟随和主动取消，这些限制不会被静默模拟。

[`examples/wasi-http-client/`](../../examples/wasi-http-client/) 提供可复制的最小应用：Calcit Snapshot、
固定发布版本依赖的 Wasmtime host，以及从 Cirru EDN contract 到 runnable Component 的完整命令。
这个示例复用上述边界和现有命令，不引入测试专用 import、源码 path dependency 或新的 CLI 包装层。

## 实施顺序

1. 导出 directional typed contract，完成确定性、数值宽度、诊断和 Cirru EDN/JSON 等价。
2. 为 Bool、Buffer、Number、明确宽度数值、String、递归同质 List、Unit 结果、闭合单态 Option/Result、Struct record 与普通 Enum variant 生成 Canonical ABI import/export adapter。
3. 由 `calcit-bindgen` 生成 WIT 并打包 runnable component，在 Wasmtime 和 jco 做端到端往返。
4. 由 Component Interface IR v3 的显式 invocation 驱动 WASI 0.3 async function adapter：已完成 export 的 `task.return`，以及 direct/indirect import 的 subtask/waitable/drop；下一步补 callback cancellation，request 路径稳定后再设计 stream。
5. 在异步基础上交付有界 buffered WASI HTTP client；stream、service 与更底层 socket 继续后置。

用户可观察的类型与语义优先由 Calcit definition `:tests` 覆盖；Rust 测试只覆盖 contract serialization、WASM encoding、Canonical ABI/memory layout 和 unsupported boundary。
