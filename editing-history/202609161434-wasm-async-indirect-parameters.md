# WASI 0.3 async indirect parameters / WASI 0.3 异步间接参数

## 中文

- WASI 0.3 async import 在参数展开不超过 4 个 flat values 时继续使用 direct shape；更宽的签名改为传递单个 parameter record pointer。
- record 复用既有 Canonical ABI value layout、对齐、lowering codec 与 checked allocator，不增加 Calcit 表层类型或 CLI。
- 集成 fixture 以两个 String、一个 Number 和一个 Bool 验证 32-byte record 的字段偏移、立即完成 return area 与 typed `task.return` 往返。
- definition `:tests` 保留用户可观察的声明契约；Rust/Node integration 进一步验证 raw core signature、memory layout、生命周期、typed `task.return`，以及 Result discriminant、文本、Bool 与 indirect combine 的实际结果值。

## English

- WASI 0.3 async imports retain the direct shape for at most four flat parameter values and pass one parameter-record pointer for wider signatures.
- The record reuses the existing Canonical ABI value layout, alignment, lowering codecs, and checked allocator without adding a surface type or CLI.
- The integration fixture uses two Strings, one Number, and one Bool to verify the 32-byte record offsets, immediate return area, and typed `task.return` round trip.
- Definition-attached tests preserve the user-visible declaration contract; the Rust/Node integration additionally verifies the raw core signature, memory layout, lifecycle, typed `task.return`, and the actual Result discriminant, text, Bool, and indirect-combine values.
