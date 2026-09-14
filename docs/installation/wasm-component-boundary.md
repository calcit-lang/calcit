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

`defwasm-import` 和 `defwasm-export` 当前使用 Calcit value ABI：

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

## 类型闭包

当前已生成 Canonical ABI adapter 的同步类型只有 `Number` 与 UTF-8 `String`。Bool、明确整数/浮点数、Buffer、List、Option、Result、Struct record 和 Enum variant 属于后续同步边界计划；在对应 adapter 实现以前，即使 contract 能表达这些类型，`calcit wasm --boundary component` 也会明确拒绝。

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

当前第一批 adapter 覆盖 `Number` 与 UTF-8 `String` 的同步 export。Number 直接使用 Canonical ABI `f64`；String 参数展开为 `(ptr,len)`，String 结果返回指向 `(ptr,len)` return area 的指针。core module 导出 `memory` 与 `cabi_realloc`，但不会携带 native core target 的隐式 `math/io` imports。Component import、Bool/整数、复合类型、post-return 与 async 仍是后续任务；不支持的 schema 在生成阶段明确失败。

## 实施顺序

1. 导出 directional typed contract，先完成确定性、诊断和 Cirru EDN/JSON 等价。
2. 为 Number 和 String 生成 Canonical ABI export adapter，再补 import，并扩展 Bool/整数、record、variant、Option 与 Result。
3. 由 `calcit-bindgen` 生成 WIT 并打包 runnable component，在 Wasmtime 和 jco 做端到端往返。
4. 同步边界稳定后，再引入 WASI 0.3 async、HTTP 和 socket。

用户可观察的类型与语义优先由 Calcit definition `:tests` 覆盖；Rust 测试只覆盖 contract serialization、WASM encoding、Canonical ABI/memory layout 和 unsupported boundary。
