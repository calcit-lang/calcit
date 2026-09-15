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
0.15.1 进一步覆盖 monomorphic Struct record 与闭合单态 Enum。

Calcit 包本身只生成供 packaging 消费的 core module，不在主仓库内生成 WIT 或包装 Component。
独立的 `calcit-bindgen` 已能为基础类型、Option/Result、Struct 与 Enum 生成 WIT、包装 runnable Component，
并通过 Wasmtime 与 jco/Node 验证。
项目仍需显式安装和调用该工具，不能把裸 core module 当作 Component 产物。

Bool 的 Canonical ABI 使用 `i32`
的 `0`/`1`；import 与 export 两个方向都会验证输入和返回值，遇到其他整数、小数、负数、
NaN 或越界数值时直接 trap，不把非规范值静默解释为真假。Buffer 映射为 WIT
`list<u8>`，按原始字节的 `(ptr,len)` 传输；空 Buffer、内嵌零和非 UTF-8 字节都必须原样保留，
越界范围直接 trap。`List<T>` 直接复用类型推导得到的 item type，不增加运行时猜测；当前可递归组合
`Bool`、`Buffer`、`Number`、UTF-8 `String` 与 `List<T>`，例如 `List<List<Number>>`。列表的
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
受检边界证明。它们的运行时表示仍是统一的 `Number`。当前 Component contract 尚未消费这些类型；
adapter 不能依据值范围或调用位置猜测宽度，也不能静默退化为 `Number`，须等待明确的 contract 与 lowering 支持。

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

明确宽度的整数/浮点数已在 0.15.2 具备 source-level refinement；Component contract 消费与
Canonical ABI lowering 仍待完成。在对应 adapter 实现以前，`calcit wasm --boundary component` 会明确拒绝。

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

- 新 Component contract 默认输出 Cirru EDN；
- `--format json` 用于需要 JSON 的 consumer；
- 现有 native raw-binding inventory 的 human 输出和 `--json` 保持兼容；
- Cirru EDN 和 JSON 必须表达同一个 versioned contract 与 revision。

生成 adapter 时继续沿用 `calcit wasm`：

```bash
calcit wasm calcit.cirru --boundary component --emit-path target/component-core
```

当前 adapter 覆盖 `Unit` 结果、`Bool`、`Buffer`、`Number`、UTF-8 `String`、递归同质
`List<T>`、闭合单态 Option/Result、monomorphic Struct record 与闭合单态 Enum 的同步 import/export。
Bool 使用 Canonical ABI `i32` 并严格限制为 `0`/`1`；Number 直接使用 `f64`。Buffer 映射到 WIT
`list<u8>`，String 映射到 WIT `string`；`List<T>` 只接受闭合、单态且 item type 已受支持的 schema，
并递归使用对应元素 layout，不把异构值或 `Dynamic` 猜成列表元素。Buffer、String 和 List 的 core 参数
都展开为 `(ptr,len)`，但 lift 后保持不同的 Calcit 类型。Option/Result 与普通 Enum 使用统一 variant layout
与 flat join；Struct 按规范化 schema field layout 递归组合相同 value shape，不为每个类型组合增加独立规则。
export 对应 `canon lift`，超过同步 Canonical ABI 单结果上限的 flat results 返回指向 return area 的 `i32`
指针；import 对应 `canon lower`，结果使用 caller 传入的 return area，再复制回对应的 Calcit 值。两者是
Canonical ABI 针对不同方向规定的函数形状，不是可以互换的自定义约定。core module 只保留显式声明的
Component imports，同时导出 `memory` 与可按需增长 memory 的 `cabi_realloc`，不会携带 native core target
的隐式 `math/io` imports。宽度明确数值类型的 Component contract/Canonical ABI lowering、post-return 与 async
仍是后续任务；不支持的 schema 在生成阶段明确失败。

## 实施顺序

1. 导出 directional typed contract，先完成确定性、诊断和 Cirru EDN/JSON 等价。
2. 为 Bool、Buffer、Number、String、递归同质 List、Unit 结果、闭合单态 Option/Result、Struct record 与普通 Enum variant 生成 Canonical ABI import/export adapter；宽度明确的数值类型继续接入 Component contract 与 lowering。
3. 由 `calcit-bindgen` 生成 WIT 并打包 runnable component，在 Wasmtime 和 jco 做端到端往返。
4. 同步边界稳定后，再引入 WASI 0.3 async、HTTP 和 socket。

用户可观察的类型与语义优先由 Calcit definition `:tests` 覆盖；Rust 测试只覆盖 contract serialization、WASM encoding、Canonical ABI/memory layout 和 unsupported boundary。
