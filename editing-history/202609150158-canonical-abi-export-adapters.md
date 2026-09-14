# Number/String Canonical ABI export adapters

- `calcit wasm --boundary component` 复用现有 WASM 入口，默认 `native` 行为保持不变。
- Component core module 只为严格类型化的 `defwasm-export` 生成 adapter；首批覆盖 `Number` 与 UTF-8 `String`，缺失 schema、非固定 arity、generic、import 和其他类型以稳定 path-aware 诊断拒绝。
- String 参数从 Canonical ABI `(ptr,len)` 复制到 Calcit tagged heap；String 结果返回指向 `(ptr,len)` return area 的 `i32`。core 同时导出共享 memory 与 `cabi_realloc`。
- Component boundary 不携带 native core target 的隐式 `math/io` import，避免 contract 外宿主能力进入待包装 module。
- `calcit-bindgen` 继续负责 WIT generation 与 runnable component packaging；core 不重新引入 WIT tooling。
- fixture 的用户可观察 Number/String 语义放在 Calcit definition `:tests`；Rust/Node integration 只验证 WASM signature、linear-memory layout、allocator 与 adapter 边界。
