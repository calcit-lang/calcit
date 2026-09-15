# Numeric refinement types

中文：

- 在统一的 `Number` 运行时表示上增加 `Int8`/`UInt8` 到 `Int64`/`UInt64` 以及 `Float32`/`Float64` 静态 refinement；只允许 refinement 向 `Number` 扩宽。
- 通过十个 `number->*` core 函数建立显式受检边界，返回类型化 `Result`，不截断、环绕或静默舍入。
- native 与 JavaScript 复用同一范围和精度语义；类型化 Cirru EDN 解码同步验证 refinement，并将 data-shape ABI 更新到 v3。
- 用户行为主要由 core definition `:tests` 覆盖，现有 `test-types` 入口补充 native/JavaScript 一致性；Component contract 与 Canonical ABI lowering 留给后续任务。

English:

- Add `Int8`/`UInt8` through `Int64`/`UInt64` and `Float32`/`Float64` static refinements over the single runtime `Number` representation; only refinement-to-`Number` widening is implicit.
- Establish explicit checked boundaries through ten `number->*` core functions that return typed `Result` values without truncation, wrapping, or silent rounding.
- Share range and precision semantics between native and JavaScript; validate refinements during typed Cirru EDN decoding and advance the data-shape ABI to v3.
- Keep observable coverage in core definition `:tests`, with the existing `test-types` entry checking native/JavaScript parity; defer Component contracts and Canonical ABI lowering to later tasks.

Verification: `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked -q`, `yarn check-all`.
