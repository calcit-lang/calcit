# 推导不再产出 Optional（#1554）

Jon 在 2026-10-20 决定“用 Option，尽量废弃 nil”。类型推导里有三处生产 `Nullable(T, NullableOrigin::Calcit)`（旧 `Optional<T>`）：无 else 的 `if`（`when` / `cond` / `case` 展开后同形）、`T` 与 `nil` 的分支合并，以及已知 struct 字段上 `.?-field` 的 `wrap_optional_type`；`wrap_option_like` 只保持已声明的 Optional 外壳，不是生产者。`T|nil` 不是 Calcit 类型，继续推成 Optional 只是在把旧表示写回新代码。

决策依据与边界：

- 前两处不再给出类型（返回“无证据”）。不推 `Dynamic`：显式 Dynamic 进入具体类型需要证明（#1538），会让原本只是缺证据的代码在严格模式下新增失败。无证据本身不会让检查收紧；但单纯“无证据”会让有值分支的载荷不再与合同比较（CI 中 `check-known-assertion` 的 `wrong-live-payload` 从失败变成通过）。因此 `type_checking::nil_branch_contradiction` 在函数返回与调用实参处，对无 else / nil 合并的 `if` 逐支比较有值分支与合同（去掉合同的可空外壳），只把确定矛盾报告为原有的 `W_FN_RETURN_TYPE_MISMATCH` / `W_*_ARG_TYPE_MISMATCH`；开放、泛型或未解析的合同保持原有迁移策略。
- `.?-field` 是 JS 可选链，结果改为 `JsNullish<T>`，与同一分支里无 struct 证据时已有的 `js_nullish_host_value_type` 一致。
- 诊断只在“值被消费”的位置报告：`&let` 初始化、调用与方法实参、声明了非 Unit/Nil/Dynamic 返回类型的函数结尾。语句位置（`&do` 中非末项、函数体前部）不报告，Unit 式 `if` 不受影响。报告集合等于过去会产出 Optional 的集合（排除 Nil/Unit/Never 分支和已是 Nullable 的分支）。`nil->option` 的实参豁免，它就是显式桥。
- 级别：实现时 main 的 Cargo 版本是 0.30.1-alpha.4 补丁线，补丁线不新增让严格检查失败的诊断；提交前 main 已切到 0.31.0-alpha.1（非 patch），升级为阻塞诊断已被允许，但需要单独决定并重新验证，本次先保持提示级别；`gen_check_warning` 在本仓库会阻塞 runner（`Found N warnings, runner blocked`），所以采用已有的 advisory 通道（与 `W_OPTION_ERASED_TO_NIL` 相同）：`--check-only` 在 Notes 中列出，检查与运行照常通过。下一个非 patch 版本可以改成阻塞诊断，不需要再改推导。
- 新编号 `W_IMPLICIT_NIL_BRANCH`：`E_NIL_FOR_UNIT` 只针对声明返回 Unit 的函数结尾，`E_NIL_CALLBACK_SENTINEL` 只针对 `map-kv` 回调，`W_OPTION_ERASED_TO_NIL` 针对已有 Option 被擦回 nil，`W_NIL_TYPE_DEBT` 属于 `analyze weak-types`；没有一个能表达“值位置的分支静默产出 nil”。
- `let` 绑定桥接：表达式推导不给类型后，`let` 绑定会落到 Dynamic，而 Dynamic 进入具体槽位需要证明（#1538），`check-known-assertion` 的 `nullable-branch-binding` 合同（可空字段接受、非可空 / Option / 错误载荷 / 开放值拒绝）因此失败。“无类型”会全部接受，Dynamic 会全部拒绝，二者都不满足该合同。所以 `preprocess_core_let` 在常规推导为空时，用 `infer_let_binding_with_legacy_nil_evidence` 以旧规则为这一个初始化式重新推导，绑定保留原 `Optional<T>` 证据；该位置仍报告 `W_IMPLICIT_NIL_BRANCH`。退场条件：0.32.0 删除 `Optional<T>`。
- WASM 标量 ABI 需要区分 nil 与 `false` / `0`。表层推导不再给出类型后，`emit_wasm` 的 `infer_wasm_static_type` 对无 else / nil 合并的 `if` 自行从有值分支推出后端布局证据 `Nullable(T)`，只在后端内部使用，不回流为表层类型；否则 `nil->option $ when …` 会在 WASM 报 `E_WASM_NIL_TYPE_EVIDENCE`。`Optional<Number>` 等零值载荷仍按原有限制报错。
- 不改 `nil->option` 签名（裸 `T` 曾破坏合法代码）；`E_LEGACY_OPTIONAL_SCHEMA` 与 Optional 的删除（0.32.0）不在本次范围。

验证：`cargo test`、`calcit/test.cirru`、core `:tests` 以及 native / JS / WASM 回放；新诊断用 Rust 的 type_fail 用例固定，Calcit 侧用 `:tests` 固定“无 else / nil 合并的值不再带 Optional 证据且仍按原值运行”。
