# 收紧 `calcit.core` 剩余公开 `Dynamic` 签名（#2842）

继 PR #2844（`385b8f30`）将 `calcit.core/None` 的 `:schema` 从 `Option Dynamic` 收紧为 `Option Never` 后，针对 #2842 追踪的其余 7 处 `migrate/public-core-api` 位置及 `calcit.test/fail` 完成收口，不新增任何 `*-dynamic` 逃生函数，`docs/core-dynamic-classification.md` 待迁移计数清零：

1. **`calcit.core/raise` 与 `calcit.test/fail`**：
   - `calcit.core/raise` 的 `:schema` 从 `Fn (& Dynamic) (-> Never)` 收紧为 `Fn M D (-> Never) (:generics M D)`，并在 `CalcitProc::Raise::optional_parameter_count` 登记 1 个可选参数（运行期 arity `1..=2`，与 JS 后端 `(raise message ?data)` 一致，同时兼容 `String` 或 `Tag` 错误标识及可选数据载荷）。
   - `calcit.test/fail` 的 `:schema` 从 `Fn String (-> Dynamic)` 收紧为 `Fn String (-> Never)`，同步移除 `scripts/core-dynamic-classification.mjs` 中早期因缺少 `Never` 而保留的 `reviewedTestPositions` 豁免项。
2. **`calcit.core/max`、`calcit.core/min` 与 `&list:numbers`**：
   - `max` 与 `min` 直接在内部单遍 `foldl` 到 `Option Number`（折叠函数签名为 `Fn (Option Number) T (-> (Option Number)) (:generics T)` 并由 `number?` 收窄），保持原有的 `List`/`Set` 接收与非数值元素报错语义不变，删除仅内部使用的过渡辅助函数 `calcit.core/&list:numbers`。
3. **`calcit.core/&map:assoc` 变参键值成对校验**：
   - 表面 schema 的单一 `:rest` 槽位无法直接表达 `K` 与 `V` 交替出现的变参对；在 `check_proc_arg_types` 与 `preprocess_list_call`（含字面量 `&` 展开后的实参列表）中将 `&map:assoc` 的尾随参数按接收者的 `(K, V)` 成对展开校验，并对不成对的偶数参数数量报 `E_PROC_ARITY`。该位置归入 `compiler-specialized-contracts`。
4. **`calcit.core/each` 与数据定义子句反射辅助函数**：
   - `each|schema.args.1.args.0` 在已知 `List`、`Set`、`Map` 接收者下由 `checked_call_contract` 特化回调入参，仅在开放接收者下保留 `Dynamic`，归入 `compiler-specialized-contracts`。
   - `data-definition-clause?` 与 `data-definition-clauses` 已标 `(:open :reflect)`，与同族 `data-definition-form` 一并归入 `open-data-boundaries`。

验证：`CALCIT_LINT_CORE=1 calcit src/cirru/calcit-core.cirru test --tag unit`（776 passed）、`calcit docs check-md docs/features/error-handling.md --snapshot calcit/test.cirru`、`node scripts/core-dynamic-classification.mjs --check`（0 待迁移）、`node scripts/core-api-contract.mjs`、`node scripts/check-strict-default.mjs`、`cargo fmt --check` 与 `cargo clippy --all-targets -- -D warnings` 全部通过。
