# 值位置的 nil 分支提示改用 Option（#1554）

Jon 在 2026-10-20 决定“用 Option，尽量废弃 nil”。类型推导里有三处生产 `Nullable(T, NullableOrigin::Calcit)`（旧 `Optional<T>`）：无 else 的 `if`（`when` / `case` 展开后同形）、`T` 与 `nil` 的分支合并，以及已知 struct 字段上 `.?-field` 的 `wrap_optional_type`。

决策依据与边界：

- 第一版直接让前两处“无证据”、`.?-field` 改推 `JsNullish<T>`，并补了逐支比较与 `let` 桥接。评审用探针（每个用例分别交给 main 与 PR 的 `--check-only`）证明这仍有回退：nil 流入非可空合同、proc / method 实参、`%{}` 字段、`assert-type`、回调与 `let` / `do` / 嵌套主体里的错误载荷，从 main 拒绝变成 PR 接受；`.?-field` 则让 `nil?`、`nil->option` 与返回 `Optional<String>` 从通过变成严格失败。逐个形状补检查覆盖不全。
- 采用方案 (a)：0.32.0 删除 `Optional<T>` 之前，推导保持 main 的旧可空合并，所有合同检查都按它进行，main 的每一个拒绝与接受都保留；`.?-field` 也保持旧结果。因此删去了第一版的逐支比较、`let` 桥接与 WASM 自行推导（推导恢复后 WASM 已有同样的证据）。本次只新增值位置提示 `W_IMPLICIT_NIL_BRANCH`，真正去掉推导中的 `Optional<T>` 与删除 Optional 同步在 0.32.0 进行。
- 诊断只在“值被消费”的位置报告：`&let` 初始化、调用与方法实参、声明了具体返回类型的函数结尾。语句位置、`nil->option` 实参、声明为可空（`JsNullish<T>` / `Optional<T>`）的参数与返回值不报告。
- 级别：实现时 main 为 0.30.1-alpha.4（patch 线，不新增让严格检查失败的诊断），采用已有的 advisory 通道（与 `W_OPTION_ERASED_TO_NIL` 相同）。main 现已是 0.31.0-alpha.1；是否升级为阻塞诊断由 Jon 决定。
- 新编号 `W_IMPLICIT_NIL_BRANCH`：`E_NIL_FOR_UNIT` 只针对声明返回 Unit 的函数结尾，`E_NIL_CALLBACK_SENTINEL` 只针对 `map-kv` 回调，`W_OPTION_ERASED_TO_NIL` 针对已有 Option 被擦回 nil，`W_NIL_TYPE_DEBT` 属于 `analyze weak-types`；没有一个能表达“值位置的分支静默产出 nil”。
- 不改 `nil->option` 签名；`E_LEGACY_OPTIONAL_SCHEMA` 与 Optional 的删除（0.32.0）不在本次范围。

验证：探针 72+ 个用例与 main 的退出码逐一一致，并固定为 `tests/fixtures/nil-branch-contracts.cirru` 与 type_fail 用例 `nil_branch_contracts_keep_every_main_rejection`；`when` / `case` 的 lowering 用例加入 `tests/post_lowering_cli.rs`（#1553）；core `nil->option` 的 `:tests` 在 native / JS / WASM 回放无 else、nil 合并、`(do nil)` 与 nil 局部经 `nil->option` 的结果。
