# 登记 Optional 的 0.32.0 删除版本，并固定 nil->option 的现有行为

## 问题与方向

`nil->option` 是 core 里最后一个以 `(Optional T)` 作为公共参数类型的定义，靠 `legacy_optional_public_schema_requires_migration` 中按名字的豁免避开 `E_LEGACY_OPTIONAL_SCHEMA`，这使 Optional 无法删除。关联 #1554。

## 尝试与结论：裸 `T` 不能单独落地

第一版把参数直接改成泛型 `T` 并删除豁免，同时为了保住“`JsNullish` 不被接受”加了一条按 core 函数名的参数检查。评审发现这会让当前通过的写法在严格检查中失败（`W_FN_RETURN_TYPE_MISMATCH`）：

- `fn (x:Number) -> Option<Number>` 内的 `nil->option $ if (> x 0) x`（无 else 的 `if` 推出 `Optional<Number>`，旧签名能把它折叠成 `Option<Number>`，裸 `T` 得到 `Option<Optional<Number>>`）；
- 形参声明为 `Optional<Number>` 并传给 `nil->option`；
- 声明 `Option<Number>` 返回、分支分别为 `nil->option x` 与 `nil->option nil` 的 `if`。

也就是说，旧的 `Optional<T>` 参数承担了“结果折叠”的类型证据，不只是接受 nil。当前 main 是 patch 线，不能新增让严格检查失败的诊断，因此本 PR 撤销了签名改动、按名字的 `reject_js_nullish_args`、豁免删除、`snapshot.rs` 测试改写、手改的 API 基线，以及放宽 `core-api-contract.mjs` 的规则（该规则把任意位置的 `(Optional X)` 与 `X` 视为相等，会放过嵌套 Fn 返回参数里的真实破坏）。参数类型改动必须与 Optional 的删除一起落地（0.32.0），或者先为 `nil->option` 增加专门的结果折叠特例；两者都属于删除 PR 的范围。

## 本 PR 保留的内容

- `E_LEGACY_OPTIONAL_SCHEMA` / `W_LEGACY_OPTIONAL_SCHEMA` 的文字加入删除版本 0.32.0，诊断编号不变，对应单测断言版本号；不新增 fix 规则。
- `docs/run/upgrade.md` 登记 0.32.0 删除项和现在可用的迁移步骤（用 `--check-only --keep-going --all-defs` 定位 `E_LEGACY_OPTIONAL_SCHEMA`，`edit transaction` 先 dry-run 再带 revision 应用，`nil->option` 为显式桥），并改写过期的“105/19”盘点；`strict-nil-migration.md`、`polymorphism.md` 同步。下游命中情况注明扫描方式与覆盖范围，未覆盖不算零。
- 固定现有行为的回归测试，供后续删除 PR 对照：`nil->option` 的 definition `:tests` 覆盖无 else `if`、`Optional<Number>` 实参、`if`/else 两个 `nil->option` 分支、类型明确的列表上 `map xs nil->option`；`type_fail.rs` 覆盖 `JsNullish` 实参仍被拒绝、nil / Option / Optional 实参无警告，以及未标注元素类型的 `map ([] 1 nil 2) nil->option` 今天报 `E_ERASED_GENERIC_RELATION`（只有已知元素类型的列表可以直接把 `nil->option` 当回调）。`JsNullish` 的拒绝只能写在 Rust：`:tests` 在检查通过后运行，无法表达预期的预处理诊断。

## 验证

见 PR 描述中的命令与结果清单。

补充：折叠形式的 `:tests` 在 WASM 上因 `E_WASM_NIL_TYPE_EVIDENCE`（标量 ABI 无法区分 nil 与零值 `Optional<Number>`）不可编译，已按既有格式登记到 `scripts/core-tests-exclusions.cirru`，native / JS 照常重放。
