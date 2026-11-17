# 202611171012-fix-wasm-dissoc-collection-parity

## 背景与目标
解决 [#2851](https://github.com/calcit-lang/calcit/issues/2851)：修复 WASM/WASI 中 `dissoc` 展开与多键删除的字节码栈平衡与 lowering，并移除 `scripts/core-tests-exclusions.cirru` 中已通过的 20 条集合 `parity:` 排除项。

## 改动摘要
1. `src/wasm/emit_wasm.rs` / `src/wasm/emit_wasm/lists.rs`：
   - 在 `emit_call_spread_args` 中，当显式实参个数 `n_explicit > fixed` 时，只先发射前 `fixed` 个固定参数，再将剩余显式实参 `&explicit_args[fixed..]` 打包为前缀列表并与 `spread_list_expr` 通过 `emit_list_concat_two_i32` 拼接成单个 rest 列表，消除 `Call` 后的残留栈元素。
   - 在 `emit_call_spread` 与 `emit_call_expr` 中，对接收者静态已知为 `Map`（或非 `List`）且参数不少于 2 个的 `calcit.core/dissoc` 调用直接委托给 `emit_map_dissoc`，支持字面量展开、运行时列表展开、多段展开与混合键值。
2. `scripts/core-tests-exclusions.cirru`：
   - 移除 `:wasm` 与 `:wasi` 下 `dissoc`、`&map:map`、`map`、`&list:map`、`destruct-map` 共 20 条已通过的 `parity:` 排除项。
