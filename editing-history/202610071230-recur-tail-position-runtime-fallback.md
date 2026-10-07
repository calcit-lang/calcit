# recur 尾部位置：统一分析与运行时兜底（#1649）

## 决策

- 尾部位置分析抽到 `src/runner/preprocess/recur_tail.rs` 的 `non_tail_recur_forms`，预处理（告警，严格检查失败）、JS codegen 与 WASM codegen（报错）共用同一条规则，不在各后端各写一份。
- 尾部位置沿 `if` 分支、`&let` 最后一项、`try` 主体、`match` 分支体（含静态 enum 的索引分支表）以及 `&call-spread recur ...` 传递；`recur` 被当作值引用（`let ((r recur))`、`apply recur xs`）同样视为逃逸。
- 宏展开后的代码再分析，所以宏生成的 `recur` 适用同一规则；`calcit.core` 不再豁免尾部检查（参数个数/类型检查的豁免保持不变）。
- native runtime 兜底：参数求值、非末尾的函数体表达式、`if` 条件、`&let` 绑定值、`match` 被匹配值、定义值以及代码中嵌入的 `Recur` 值，遇到 `Recur` 即报错。
- JS 的 `CalcitRecur` 只由尾递归循环模板消费，WASM 把 `recur` 降为跳回循环开头；两者在 codegen 阶段拒绝非尾部 `recur`，避免生成泄漏数据或静默重启循环的代码。

## 兼容边界

此前通过检查的 `try` / `match` 内部非尾部 `recur`、以及把 `recur` 当作值传递的写法，现在检查失败。这些写法原本的运行结果就是错误的（`Recur` 被当作数据或被丢弃），没有自动 fix。

## 验证

- `tests/fixtures/non-tail-recur.cirru` 与 `recur_outside_tail_position_is_rejected_by_the_check`：尾部 `loop` / `try` / `match` 通过，列表、`str`、非末尾、`try`、`match`、别名与宏生成的非尾部写法失败。
- `calcit.core/recur` 的 `:tests`（`:unit`）：`try` 与 `match` 中的尾部 `recur` 正常循环。
- Rust：`escaping_recur_values_are_rejected_at_runtime`（手工构造绕过静态检查的代码）、`rejects_recur_outside_tail_position`（WASM）。
