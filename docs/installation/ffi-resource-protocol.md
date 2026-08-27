---
title: "FFI opaque resource protocol / FFI 不透明资源协议"
scope: "core"
kind: "reference"
category: "installation"
aliases:
  - "ffi resource"
  - "opaque resource"
---

# FFI opaque resource protocol / FFI 不透明资源协议

## 中文

同步 buffer v1 只传递 Cirru EDN 字节，普通 `AnyRef` 不能跨动态库边界。resource v1 在不暴露 Rust layout、trait object、allocator ownership 或裸对象地址的前提下，让模块返回带自动析构语义的 native 对象。编译后的正则表达式是首个使用场景。

### 模块导出

使用 resource v1 的动态库必须在同步 buffer v1 导出之外提供：

```rust
#[unsafe(no_mangle)]
pub extern "C" fn calcit_ffi_resource_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn calcit_ffi_resource_release_v1(
  handle: u64,
  generation: u64,
) -> i32 {
  // 0 表示释放成功；非 0 是可诊断的模块状态码。
  0
}
```

创建资源的方法仍返回普通 buffer v1 Cirru EDN。资源位置使用保留结构：

```cirru-edn
%{} 'CalcitFfiResourceV1 (:token $ buf 01 00 00 00 00 00 00 00 01 00 00 00 00 00 00 00)
```

`:token` 必须是恰好 16 字节的 buffer：前 8 字节是 little-endian `u64` handle，后 8 字节是 little-endian `u64` generation；两者都必须非零。该结构可以嵌套在 list、set、map、enum、struct 或 atom 中。

### 生命周期与所有权

- 模块拥有 registry 中的 native 对象，宿主只保存 `(module, handle, generation)`。
- Calcit 将返回 token 转换为宿主管理的 `AnyRef`，并在资源存活期间固定创建它的 dylib。
- Calcit 值的 clone 只共享宿主引用，不调用模块 retain；最后一个宿主引用析构时 exactly-once 调用 `calcit_ffi_resource_release_v1`。
- `release` 可能在任意宿主线程执行，模块实现必须线程安全、不得 panic 或跨 FFI unwind。
- 模块必须用 generation 拒绝 stale handle，并确定性处理未知或重复释放的 token。
- 只有创建资源的同一个模块可以接收该资源参数。宿主在调用前拒绝 wrong-module、普通 AnyRef 和用户直接构造的保留 token。
- 普通 Cirru EDN 格式化不会序列化资源；仅同模块的 buffer v1 adapter 会临时还原 token。
- `calcit --trace-ffi` 会记录 `resource-create` 与 `resource-release` 的模块、handle、generation 和 release 状态，不记录对象内容。

不需要也不应向 Calcit 用户暴露手动 `drop`。模块 registry 可以在 dylib shutdown 时报告剩余槽位，但正常资源释放由宿主引用计数驱动。

### 模块 registry 建议

registry 应放在同步锁之后，并为每个槽位维护 generation。复用空槽位时递增 generation；generation 溢出时永久退役该槽位。每次方法调用和 release 都同时校验 handle 与 generation。不要把 Rust 对象地址编码成 handle，也不要让 `Vec`、`Arc`、`Box` 或 `Result` 穿过 C ABI。

## English

Synchronous buffer v1 transports only Cirru EDN bytes, so an ordinary `AnyRef` cannot cross a dylib boundary. Resource v1 lets a module return native objects with automatic destruction without exposing Rust layouts, trait objects, allocator ownership, or raw object addresses. Compiled regular expressions are the first adopter.

### Module exports

A resource-v1 dylib exports these symbols in addition to synchronous buffer v1:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn calcit_ffi_resource_version() -> u32 { 1 }

#[unsafe(no_mangle)]
pub extern "C" fn calcit_ffi_resource_release_v1(
  handle: u64,
  generation: u64,
) -> i32 {
  // 0 means released; nonzero values are diagnosable module status codes.
  0
}
```

A resource-creating method still returns ordinary buffer-v1 Cirru EDN. A resource position uses this reserved struct:

```cirru-edn
%{} 'CalcitFfiResourceV1 (:token $ buf 01 00 00 00 00 00 00 00 01 00 00 00 00 00 00 00)
```

`:token` must be exactly 16 bytes: a little-endian `u64` handle followed by a little-endian `u64` generation. Both must be nonzero. The struct may be nested in lists, sets, maps, enums, structs, or atoms.

### Lifecycle and ownership

- The module owns native objects in its registry; the host stores only `(module, handle, generation)`.
- Calcit converts a returned token into a host-managed `AnyRef` and pins its creating dylib while the resource is alive.
- Cloning a Calcit value shares the host reference and does not call a module retain function. Dropping the final host reference calls `calcit_ffi_resource_release_v1` exactly once.
- `release` may run on any host thread. It must be thread-safe and must not panic or unwind across FFI.
- The module validates generation on every call, rejects stale handles, and handles unknown or duplicate releases deterministically.
- Only the creating module may receive the resource as an argument. Before invocation, the host rejects wrong-module resources, ordinary AnyRefs, and directly forged reserved tokens.
- Ordinary Cirru EDN formatting never serializes a resource. Only the matching module's buffer-v1 adapter temporarily restores its token.
- `calcit --trace-ffi` records module, handle, generation, and release status for `resource-create` and `resource-release` events without logging object contents.

Calcit users neither need nor receive a manual `drop` API. A module may diagnose remaining registry slots during dylib shutdown, while normal release follows host reference ownership.

### Recommended module registry

Protect the registry with a synchronization primitive and keep a generation for every slot. Increment generation when reusing an empty slot; permanently retire a slot if its generation would overflow. Validate both handle and generation on every method call and release. Do not encode Rust object addresses as handles, and never pass `Vec`, `Arc`, `Box`, or `Result` through the C ABI.
