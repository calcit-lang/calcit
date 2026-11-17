# 202611171012-fix-wasm-dissoc-collection-parity

## 背景与目标
解决 [#2851](https://github.com/calcit-lang/calcit/issues/2851)：修复 WASM/WASI 中 `dissoc` 展开与多键删除的字节码栈平衡与 lowering，并移除 `scripts/core-tests-exclusions.cirru` 中已通过的 20 条集合 `parity:` 排除项。

## 决策依据
1. **为什么 `dissoc` 展开调用此前在 WASM 失败**：
   - `calcit.core/dissoc` 定义体内部包含针对动态索引的 `(number? k)` 判断，被标记进 `nil_sensitive_defs`；当 `dissoc` 通过含 `&` 展开的形式调用时，预处理阶段保留 `(&call-spread calcit.core/dissoc ...)` 而未特化为 `&map:dissoc`，随后在 `emit_call_spread` 中被 `reject_nil_specialized_spread_call` 拒绝。
   - 同时，`emit_call_spread_args` 在处理 `n_explicit > fixed` 或 `& spread` 后继续跟随额外参数/多段展开时，此前会把全部 `explicit_args` 加上 `spread_list_expr` 压栈（且丢弃 `& spread` 之后的尾部参数），导致调用 `fixed + 1` 元数的目标函数后栈上残留多余元素（`Compiling function #65 failed: expected 1 elements on the stack for fallthru, found 2`）。
2. **为什么只对静态已知 `Map` 接收者直接委托 `emit_map_dissoc`**：
   - 公开 `dissoc` 同时支持 `List`（按单个数值下标删除）与 `Map`（按一个或多个键删除）。若对未知/`Dynamic` 接收者盲目走 `emit_map_dissoc`，持有 `List` 的动态值会被当作 Map 指针传给 `__rt_map_dissoc` 造成错误结果。
   - 因此在 `emit_call_expr` 与 `emit_call_spread` 中，仅当 `infer_wasm_static_type` 证明接收者为 `Map(_, _)` 时才直接路由到 `emit_map_dissoc`；同时在 `emit_map_dissoc` 中对非静态 `Map` 接收者增加运行期堆标记与 `"map"` tag 校验（非 Map 直接 `unreachable` trap），防止底层 `&map:dissoc` 在开放边界被误用。

## 改动摘要
1. `src/wasm/emit_wasm.rs` / `src/wasm/emit_wasm/lists.rs` / `src/wasm/emit_wasm/maps.rs`：
   - 新增 `emit_packed_spread_args_list` 并重构 `emit_call_spread_args`：先发射前 `fixed.min(pos)` 个固定实参，再将剩余参数切片（含多余显式实参、一个或多个 `& spread` 列表以及 `& spread` 之后的尾部普通实参）按源码顺序求值并通过 `emit_list_concat_two_i32` 拼接为单个列表；无论 `n_from_spread == 0` 还是 `n_from_spread > 0` 均保证栈平衡且不丢失尾部参数。
   - 在 `emit_call_spread` 与 `emit_call_expr` 中，仅当接收者静态已知为 `Map(_, _)` 且参数不少于 2 个时将 `calcit.core/dissoc` 委托给 `emit_map_dissoc`；在 `emit_map_dissoc` 中为非静态 `Map` 接收者补齐运行期 `"map"` tag 检查。
2. `scripts/core-tests-exclusions.cirru`：
   - 移除 `:wasm`（10 条）与 `:wasi`（10 条）共 20 条已通过四后端回放的集合 `parity:` 排除项（未改动任何 `cos`/`pow`/`sin` 等其他条目）：
     - `:wasm`：`&map:map#maps-map-pairs-directly`、`&map:map#infers-result-keys-and-values-from-the-pair`、`dissoc#empty-list-spread-keeps-map`、`dissoc#explicitly-converts-set-keys`、`dissoc#preserves-unspread-arguments`、`dissoc#removes-multiple-map-keys`、`dissoc#spreads-list-keys`、`dissoc#supports-multiple-list-spreads`、`map#maps-map-pairs`。
     - `:wasi`：`&list:map#typed-empty-accumulator`、`&map:map#maps-map-pairs-directly`、`&map:map#infers-result-keys-and-values-from-the-pair`、`destruct-map#splits-entry-and-remainder`、`dissoc#empty-list-spread-keeps-map`、`dissoc#explicitly-converts-set-keys`、`dissoc#preserves-unspread-arguments`、`dissoc#removes-multiple-map-keys`、`dissoc#spreads-list-keys`、`dissoc#supports-multiple-list-spreads`、`map#maps-map-pairs`。

## 兼容边界
- 静态已知为 `Map` 的 `dissoc` 普通调用与 `&` 展开调用（单键、多键、多段展开、运行时列表展开、前置键配空列表展开）在 Native、JS、WASM、WASI 四后端行为完全一致。
- 静态已知为 `List` 且带单个 `Number` 索引的 `dissoc` 继续由预处理特化为 `&list:dissoc`。
- 开放 `Dynamic` 接收者上的 `dissoc` 不走 `emit_map_dissoc` 旁路；直接调用 `&map:dissoc` 若传入非 Map 堆值或标量会在运行期经 tag 检查确定性 trap。

## 验证方式
- `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings`
- `node scripts/check-spread-boundary.mjs`
- `node scripts/run-core-tests.mjs --backend wasm --target calcit.core/dissoc --target 'calcit.core/&map:map' --target calcit.core/map --target 'calcit.core/&list:map' --target calcit.core/destruct-map`
- `WASMTIME_CLI=/opt/homebrew/bin/wasmtime node scripts/run-core-tests.mjs --backend wasi --target calcit.core/dissoc --target 'calcit.core/&map:map' --target calcit.core/map --target 'calcit.core/&list:map' --target calcit.core/destruct-map`
