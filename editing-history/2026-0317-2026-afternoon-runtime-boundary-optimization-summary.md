# 2026-03-17 下午至晚间改动总结（1739-2023）

## 总览

- 时间段：`2026-0317-1739` 至 `2026-0317-2023`
- 主要改动：测试减法、program/preprocess 架构减法、类型注解热路径优化、samply 验证、计划文档同步

## 关键变更（按时间顺序）

### 1) 测试减法（1739 / 1745）

- 清理 `src/program/tests.rs` 中 helper/重复语义测试：
  - `runtime_snapshot_fallback_only_allows_runtime_only_defs`
  - `preprocess_ns_def_accepts_compiled_only_value_without_source_lookup`
- 保留行为级回归用例，继续覆盖 snapshot/runtime-only 与 compiled-only 消费边界。

### 2) preprocess 推断逻辑去重（1751）

- 文件：`src/runner/preprocess.rs`
- 抽取 `infer_return_type_from_compiled_callable(...)`，统一 `Import`/`Symbol` 分支的 compiled callable 返回类型推断。
- 保留 `Symbol` 分支对源码 tag 的回退解析行为。

### 3) program snapshot helper 内联（1753）

- 文件：`src/program.rs`
- 删除并内联单次用途 helper：
  - `collect_referenced_compiled_def_ids(...)`
  - `should_use_runtime_snapshot_fallback(...)`
- 逻辑并入 `collect_snapshot_fill_tasks(...)` 与 `build_snapshot_fill_compiled_def(...)`。

### 4) 文档刷新 + 首轮热路径减法（1812）

- 文件：`drafts/runtime-boundary-refactor-plan.md`
  - 修正过时描述：`run_program_with_docs` 现状与 preprocess 返回值关系。
  - 将阶段状态与 `samply` 观察、下一步优先级对齐。
- 文件：`src/runner/preprocess.rs`
  - 去除 `drop_left` 中间列表分配。
  - `resolve_generic_return_type` 改为接收迭代器，调用处直接 `iter().skip(1)`。

### 5) materialize executable fast path（2012）

- 文件：`src/program.rs`
- `materialize_compiled_executable_payload(...)`：
  - `Proc | Syntax` 直接返回 `preprocessed_code`。
  - `Fn | Macro` 继续 `evaluate_expr` materialize。
  - `LazyValue | Value` 继续保持不可执行语义。
- 删除无调用 helper：`with_compiled_executable_payload(...)`。

### 6) type-annotation 单次扫描收敛（2023）

- 文件：`src/calcit/type_annotation.rs`
- 在 `parse_fn_annotation_from_schema_form` 中引入 `collect_fn_schema_fields`，由多次 key 扫描改为一次遍历收集。
- 删除无用 helper：`schema_has_any_field`。

## 验证汇总

- 多轮定向测试均通过：
  - `cargo test -q program::tests`
  - `cargo test -q runner::preprocess::tests`
  - `cargo test -q calcit::type_annotation::tests`
- 全量 Rust 测试持续通过：`cargo test -q`（会话末保持全绿）
- 语义门禁持续通过：`yarn check-all`

## profiling 结论汇总

- 使用既有流程：`profiling/samply-once.sh` + `profiling/samply-summary.py`
- 在 materialize 目标链路过滤中，样本权重由 14 降至 7（单轮观测，方向符合预期）。
- `type_annotation::*` 热点仍可见，后续应继续压缩 schema key 匹配/提取路径分配与分支成本。

## 本轮经验

- 以“删 helper/删重复分支/删中间分配”为主线做减法，优先保证行为级测试覆盖。
- 每次改动后固定执行“定向测试 → 全量 Rust → `yarn check-all`”可有效阻断语义回退。