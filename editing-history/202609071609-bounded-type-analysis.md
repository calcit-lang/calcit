# Bound high-risk type analysis paths / 有界化高风险类型分析路径

## Summary / 概要

- Replaced recursive generic substitution and type-variable scans with explicit worklists. Substitution caches results by source-node identity so shared subtype DAGs keep `Arc` sharing instead of being rebuilt repeatedly.
- 将递归的泛型替换和类型变量扫描改为显式 worklist；替换过程按源节点身份缓存结果，确保共享子类型 DAG 继续复用 `Arc`，而不是重复重建。
- Added an iterative compatibility fast path for List, Map, Option/Optional, Ref, Set, nominal TypeRef, Struct, and Enum layers, with a 16,384-node budget that rejects oversized relations rather than widening them to Dynamic.
- 为 List、Map、Option/Optional、Ref、Set、具名 TypeRef、Struct 和 Enum 层增加迭代兼容检查快速路径；16,384 节点预算超限时稳定拒绝，不回退 Dynamic。
- Guarded schema-alias and type-slot expansion against recursive symbolic cycles. Diagnostic rendering now truncates nested types after 32 levels with an explicit ellipsis marker.
- 为 schema alias 与 type-slot 展开增加符号环保护；类型诊断在 32 层后使用明确省略号截断。
- Corrected the misleading `CalcitStructDef::index_of` binary-search comment. Release measurements for 10,000 worst-position lookups were 3.3/12.7/31.8 ms at 32/256/1,024 fields, so this batch keeps the declaration-order scan instead of changing the public struct layout for a cache.
- 修正 `CalcitStructDef::index_of` 误导性的二分查找注释。Release 模式下 10,000 次最坏位置查询在 32/256/1,024 字段时约为 3.3/12.7/31.8 ms，因此本批保留声明顺序扫描，不为缓存改变公共 struct 布局。

## Evidence / 证据

- Rust unit coverage includes 32/256/2,048-deep Map, Option, and nominal relations; 2,048-deep substitution/scan/rendering; shared-DAG identity; oversized relation rejection; recursive type-slot cleanup; and a real program registry alias cycle.
- Rust 单测覆盖 32/256/2,048 层 Map、Option 与具名关系，2,048 层替换/扫描/渲染，共享 DAG 身份，超预算拒绝，递归 type-slot 清理，以及真实 program registry alias 环。
- `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `yarn check-all` pass across native, JavaScript, IR, WASM, static lowering, agent interface, and quality gates.
- `cargo test`、`cargo clippy --all-targets -- -D warnings` 与 `yarn check-all` 全部通过，覆盖 native、JavaScript、IR、WASM、静态 lowering、Agent interface 与质量门禁。
- Latest Calcium `main` at `5b63382` passes the deterministic diff/patch workload with the branch compiler. The real server listens on 5021, reads persisted EDN, and completes a browser WebSocket connect/disconnect; Vite 8.0.5 renders the login page without browser errors.
- 最新 Calcium `main`（`5b63382`）使用本分支编译器通过确定性 diff/patch workload；真实服务端监听 5021、读取持久化 EDN，并完成浏览器 WebSocket 连接/断开；Vite 8.0.5 登录页正常渲染且无浏览器 error。

## Follow-up boundary / 后续边界

This is the first implementation slice of #845. A dedicated structured complexity diagnostic and broader relation-context caching remain in the issue; this commit does not claim to close it.

这是 #845 的第一批实现。专用的结构化复杂度诊断与更广泛的关系上下文缓存仍留在该 Issue；本提交不宣称关闭 #845。
