# Component post-return reclamation / Component post-return 回收

## 中文

- 同步 Component export 为拥有 canonical memory 的结果生成 `cabi_post_<export>`，按闭合 schema 递归释放 String、Buffer、List、Struct、Enum、Option 与 Result；variant 只访问当前 discriminant 的 payload。
- Canonical ABI 不允许 `async` lift 同时配置 post-return。stackful 与 stackless async export 改为在 `task.return` 完成 lift 后执行同一套递归回收；取消路径只释放未初始化的 return area 与 callback state。
- String 与 Buffer 的 result lowering 复制到 canonical allocation，避免 post-return 错误释放 Calcit 内部对象。`cabi_realloc` 增加带 header 的 free-list 与容量/对齐检查，使重复调用复用已释放区域。
- free-list head 使用独立 mutable global，不能占用 linear memory 地址 0；WASI runtime 会把低地址作为 scratch memory 使用。
- ABI ownership 与 allocator 行为继续由 Rust、Node 和 Wasmtime 集成测试覆盖；现有 definition `:tests` 保留对 Calcit 表层语义的验证。

## English

- Generate `cabi_post_<export>` for synchronous Component results that own canonical memory, recursively dropping String, Buffer, List, Struct, Enum, Option, and Result values from the closed schema; variants inspect only the active discriminant payload.
- Canonical ABI forbids combining an `async` lift with post-return. Stackful and stackless async exports therefore run the same recursive reclamation after `task.return` has completed lifting; cancellation releases only the uninitialized return area and callback state.
- Copy String and Buffer results into canonical allocations so post-return never frees Calcit's internal objects. Extend `cabi_realloc` with allocation headers, a free list, and capacity/alignment validation so repeated calls reuse released regions.
- Keep the free-list head in a dedicated mutable global rather than linear-memory address zero, which the WASI runtime uses as scratch memory.
- Keep low-level ABI ownership and allocator coverage in Rust, Node, and Wasmtime integration tests while retaining existing definition `:tests` for Calcit surface semantics.

## Verification / 验证

- `cargo fmt`
- `cargo check --all-targets`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`
