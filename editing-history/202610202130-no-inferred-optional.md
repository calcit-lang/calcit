# 推导不再产出 Optional（#1554）

Jon 在 2026-10-20 决定“用 Option，尽量废弃 nil”。类型推导里有三处生产 `Nullable(T, NullableOrigin::Calcit)`（旧 `Optional<T>`）：无 else 的 `if`（`when` / `cond` / `case` 展开后同形）、`T` 与 `nil` 的分支合并，以及已知 struct 字段上 `.?-field` 的 `wrap_optional_type`；`wrap_option_like` 只保持已声明的 Optional 外壳，不是生产者。`T|nil` 不是 Calcit 类型，继续推成 Optional 只是在把旧表示写回新代码。

决策依据与边界：

- 前两处不再给出类型（返回“无证据”）。不推 `Dynamic`：显式 Dynamic 进入具体类型需要证明（#1538），会让原本只是缺证据的代码在严格模式下新增失败。无证据与旧 Optional 相比只放宽，不收紧。
- `.?-field` 是 JS 可选链，结果改为 `JsNullish<T>`，与同一分支里无 struct 证据时已有的 `js_nullish_host_value_type` 一致。
- 诊断只在“值被消费”的位置报告：`&let` 初始化、调用与方法实参、声明了非 Unit/Nil/Dynamic 返回类型的函数结尾。语句位置（`&do` 中非末项、函数体前部）不报告，Unit 式 `if` 不受影响。报告集合等于过去会产出 Optional 的集合（排除 Nil/Unit/Never 分支和已是 Nullable 的分支）。`nil->option` 的实参豁免，它就是显式桥。
- 级别：实现时 main 的 Cargo 版本是 0.30.1-alpha.4 补丁线，补丁线不新增让严格检查失败的诊断；提交前 main 已切到 0.31.0-alpha.1（非 patch），升级为阻塞诊断已被允许，但需要单独决定并重新验证，本次先保持提示级别；`gen_check_warning` 在本仓库会阻塞 runner（`Found N warnings, runner blocked`），所以采用已有的 advisory 通道（与 `W_OPTION_ERASED_TO_NIL` 相同）：`--check-only` 在 Notes 中列出，检查与运行照常通过。下一个非 patch 版本可以改成阻塞诊断，不需要再改推导。
- 新编号 `W_IMPLICIT_NIL_BRANCH`：`E_NIL_FOR_UNIT` 只针对声明返回 Unit 的函数结尾，`E_NIL_CALLBACK_SENTINEL` 只针对 `map-kv` 回调，`W_OPTION_ERASED_TO_NIL` 针对已有 Option 被擦回 nil，`W_NIL_TYPE_DEBT` 属于 `analyze weak-types`；没有一个能表达“值位置的分支静默产出 nil”。
- WASM 标量 ABI 需要区分 nil 与 `false` / `0`。表层推导不再给出类型后，`emit_wasm` 的 `infer_wasm_static_type` 对无 else / nil 合并的 `if` 自行从有值分支推出后端布局证据 `Nullable(T)`，只在后端内部使用，不回流为表层类型；否则 `nil->option $ when …` 会在 WASM 报 `E_WASM_NIL_TYPE_EVIDENCE`。`Optional<Number>` 等零值载荷仍按原有限制报错。
- 不改 `nil->option` 签名（裸 `T` 曾破坏合法代码）；`E_LEGACY_OPTIONAL_SCHEMA` 与 Optional 的删除（0.32.0）不在本次范围。

验证：`cargo test`、`calcit/test.cirru`、core `:tests` 以及 native / JS / WASM 回放；新诊断用 Rust 的 type_fail 用例固定，Calcit 侧用 `:tests` 固定“无 else / nil 合并的值不再带 Optional 证据且仍按原值运行”。
