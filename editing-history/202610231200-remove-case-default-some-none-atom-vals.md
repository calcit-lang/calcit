# 删除 case-default、%some/%none、atom 与 vals（#2079 第五批）

关联 #2079。0.31.0 从 `calcit.core` 删除已弃用的 `case-default`、`%some`、`%none`、`atom` 与 `vals`。下游调用已全部迁移并合并：Respo#295、respo-ui#124、js-ffi#186、reel#86、recollect#102、cumulo-reel#73。`%ok` / `%err` 不在本批，继续保留。

`%some` / `%none` 在编译器内部仍被用来生成 Option 值（`get` 缺失、省略的 Option 结构体字段、类型推断对空值的识别、JS 代码生成的尾部 Option 填充）。这些位置改为与 `Result :ok` 相同的 `generated_core_call` 形式，直接生成 `Option :some x` / `Option :none`（lowering 为 `NativeNamedEnumNew`），类型推断、`reset!` 擦除判定、`option:fold` 接收者识别和 `query` 的类型推导同步识别该形式；之后才删除两个 core 定义。`Option :none` 的空值 fallback 证明（`core-option-method-v1`）沿用同一判断。

`atom` 是 reader 直接解析的内建 Proc，Proc 名改为 `ref`，JS runtime 不再导出 `atom`（`js-ref.mts`、`js-cirru.mts`、`check-js-runtime-identity.mjs` 一并改为 `ref`）。EDN 的 `atom` 编码标签是另一回事，保持不变。`vals` 删除后，`distinct-values` 原先转发 `vals` 的实现改为 `&list:to-set $ &map:vals x`。`case-default` 宏及其 `core-tests` 排除项、质量配置项、`effects_graph` 的子句识别一并删除。

`calcit-core.cirru` 与所有 Snapshot 夹具只通过 calcit CLI 修改：`fix` 在 core 上因已有的 `decode-map-as` 宏输入语法错误无法通过分阶段校验，所以用 `query def --format json` 生成 `edit transaction`（`tree replace` / `add-test --overwrite` / `examples`）完成约 156 处 `%some` / `%none` 与 `atom` 的迁移。

迁移规则：`case-default-to-match-v1` 与 `core-function-alias-v1`（只剩 `vals`）随旧名整条退役，不保留空壳；`core-nominal-constructor-v1` 只保留 `%ok` / `%err`；`core-ref-constructor-v1` 只保留 `defatom` → `defref`（`atom` 已不存在）。`core-api-0.29-v1` 仍是 2 条规则。需要自动迁移的项目在升级前用 0.30.x 的 CLI 运行；`docs/run/upgrade.md` 增加了删除表、迁移命令和历史表标注。`fix/case_default.rs` 中与 case-default 无关的宏上下文判断保留，改名为 `fix/macro_context.rs`，供缺失值规则继续使用。

残留调用沿用 `retired_core_function_migration`：检查时报告未知名字，告警末尾给出删除版本和改写方式，覆盖裸名、`:refer`、`:as` 与 `calcit.core/` 前缀四种写法（`type_fail.rs` 的用例表新增五行）。

## English

Related to #2079. 0.31.0 removes the deprecated `case-default`, `%some`, `%none`, `atom` and `vals` from `calcit.core`; the downstream callers are merged (Respo#295, respo-ui#124, js-ffi#186, reel#86, recollect#102, cumulo-reel#73). `%ok` / `%err` are out of scope and stay.

The compiler itself used `%some` / `%none` to build Option values (missing `get`, omitted Option struct fields, empty-Option detection in type inference, trailing-Option fill in the JS emitter). Those sites now use `generated_core_call` like `Result :ok` and emit `Option :some x` / `Option :none` (lowered to `NativeNamedEnumNew`); type inference, `reset!` erasure, `option:fold` receiver detection and `query` type inference recognise the form before the two core definitions were deleted. `atom` was a reader-resolved Proc and is now only `ref`; the JS runtime no longer exports `atom` (the EDN `atom` tag is unrelated). `distinct-values` no longer forwards to `vals` and is `&list:to-set $ &map:vals x`.

`calcit-core.cirru` and every Snapshot fixture were edited only through the calcit CLI. `fix` cannot stage-validate core because of the pre-existing `decode-map-as` macro syntax error, so `edit transaction` files were generated from `query def --format json` (about 156 `%some` / `%none` and `atom` sites).

Rules: `case-default-to-match-v1` and `core-function-alias-v1` (only `vals` was left) are retired outright; `core-nominal-constructor-v1` keeps `%ok` / `%err`; `core-ref-constructor-v1` keeps `defatom` to `defref`. `core-api-0.29-v1` still has 2 rules. Projects migrate with a 0.30.x CLI before upgrading (commands in `docs/run/upgrade.md`). The macro-context helper that lived in `fix/case_default.rs` is kept as `fix/macro_context.rs` for the absence rules. Residual calls get the unknown-name warning for bare, `:refer`, `:as` and `calcit.core/` spellings (five new rows in the `type_fail.rs` table).
