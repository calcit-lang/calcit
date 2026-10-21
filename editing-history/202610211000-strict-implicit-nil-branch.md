# 值位置的 nil 分支改为严格错误（#1554）

#2228 合入了 `W_IMPLICIT_NIL_BRANCH` 提示。Jon 在 2026-10-21 决定“改成严格错误”。main 已是 0.31.0-alpha.1（非 patch 线），可以新增让严格检查失败的诊断。

决策依据与边界：

- 严格模式在调用 / 方法实参与声明具体返回类型的函数结尾报 `E_IMPLICIT_NIL_BRANCH`（`CalcitErrKind::Type`，与 `E_MAP_KV_UNPROVEN_CONTRACT` 同样带调用栈与位置）；`--compat-types` 下保留 `W_IMPLICIT_NIL_BRANCH` 提示。`W_IMPLICIT_NIL_BRANCH` 只在未发布的 main 上存在过，不需要墓碑或别名。
- 触发位置与 #2228 相同；语句位置、`nil->option` 实参、声明可空（`JsNullish<T>` / 旧 `Optional<T>`）的参数与返回、`Dynamic` 返回、`calcit.core` 仍不报告；完全由宏写出的分支（`if` 及其操作数都来自宏）跳过，库宏把调用者的表达式包进无 else 的 `if`（如 `defmacro maybe (c x) (quasiquote (if ~c ~x))` 用作 `str |x (maybe flag |b)`）在调用处报告，修改方式是 `nil->option (maybe flag |b)` 后按 Option 处理，或补 else 值。
- `let` 初始化保持提示：绑定后的值可以再用 `nil?` 收窄或交给可空参数（探针 `let-bound-nilq` 是合法程序），而每一处非可空使用已在使用点被拒绝；在绑定处报错会拒绝这些合法程序，且对被拒绝的程序只是多报一处。
- 推导与合同检查仍保持 main 的旧可空合并（方案 (a)），所以严格错误只会多拒绝，不会让 main 拒绝的程序通过。

测量（main 4fe94e1 与本分支逐一 `--check-only`）：

- 仓库内：`calcit/*.cirru`、`tests/fixtures/*.cirru` 无新增失败；core 与 docs check-md（78 个文件）均通过。
- 下游 36 个仓库：只有 Triadica/triadica-space@6848ccc 由通过变为失败，位置 `triadica.comp.drag-point/comp-drag-point`（`:on-mousemove $ if-not ignore-moving? (fn …)` 作为 `{}` 实参）。
- 探针 72 例（`tests/fixtures/nil-branch-contracts.cirru`）：main 的拒绝全部保留；新增拒绝 `ret-nested-if`、`ret-nested-if-else-nil`、`arg-defn-nested`、`list-literal`、`list-ret-ok`；`let-bound-nilq`、`n3-let-value` 只提示。type_fail 表格按（main 是否拒绝、现在是否拒绝、报告的编号）固定。

合并后复查（后续 PR）：

- 文档原写“库宏输出不报告”，与代码不符：代码只跳过操作数全部来自宏的 `if`。保留代码，改正 upgrade.md、strict-nil-migration.md 与本条记录，并用 type_fail 测试固定上面的 `lib.core` 宏复现。
- 没有默认分支的 `case` / `cond` 展开为 `raise`，不会得到 nil；诊断原因与文档不再列出 `case`。
- 补充 Triadica `comp-drag-point` 的具体改法（else 放空处理函数，或按条件 `assoc` 键；`if ignore-moving? nil (fn …)` 同样被拒绝），以及两条限制：实参里的 `nil? (if flag 1)` 仍是严格错误，直接交给 JS FFI 实参的无 else 分支需要 else 值或 `JsNullish<T>` 形参。行为不变。
