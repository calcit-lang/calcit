# 泛型与 `&merge` 的 union 约束不再受实参顺序影响（#2608）

`merge-maps ({} (:width 10)) overrides`（`overrides: Map Tag Scalar`，`Scalar = Number | String`）报 `W_FN_ARG_TYPE_MISMATCH`，交换两个实参就通过。core `&merge` 同样（`W_PROC_ARG_TYPE_MISMATCH`）。

原因：`CallTypeProof::prepare_inputs` 已经从所有实参求出共同上界 `V = Scalar`，但 `check_arg_types_loop` 里逐个实参推进的 `bindings` 从空开始，第一个实参先把 `V` 定成 `Number`，第二个实参再按 `Map Tag Number` 检查。proc 路径（`check_proc_arg_types`）完全没有 prepare 这一步。

- `CallTypeProof` 记录 `widened_inputs`：prepare 选出的上界严格宽于某个实参的证据时才记。`widened_input_binding` 返回这个上界。
- `check_arg_types_loop` 用它预置 `bindings`；`check_proc_arg_types` 新增 `proc_widened_peer_bindings`，对 proc 签名里的类型变量做同样的 prepare 后预置。
- 只有能接受全部实参证据的上界才用；没有放宽时行为不变。实参证明（`proof.prove`）照旧检查方差，`Ref` 仍然不变（`same-ref (Ref Number) (Ref Scalar)` 仍报错，只是报在第 1 个实参）。
- 顺带：proc 调用的告警里 `in call at app.main/??` 改为所在定义名（取 `preferred_source_call_location` 的 def）。

验证：`tests/strict_check_cli.rs::later_union_argument_widens_generic_binding`，覆盖两种顺序都通过、真实不匹配（`Map Tag String`）和 `Ref` 不变性仍报错。
