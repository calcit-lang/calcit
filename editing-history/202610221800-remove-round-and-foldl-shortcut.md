# 删除 round? 与 foldl-shortcut（#2079 第四批）

关联 #2079。0.31.0 从 `calcit.core` 删除已弃用的函数 `round?` 与 `foldl-shortcut`。下游核对：Respo、Respo UI、Reel、Markdown、Recollect、lilac-parser、ws-edn、cumulo-reel、js-ffi、std 的默认分支在 `.cirru` 源码、附带测试和示例中都没有调用这两个名字；Recollect 只在 `WASM-ROADMAP.md` 与 `plan/runtime-blockers.md` 的历史说明中提到 `foldl-shortcut`。这是文本扫描，不是 `analyze weak-types` 的清零证据。

`round?` 原是 reader 直接解析的内建 Proc，`integer?` 只是转发它的 `defn`。删除旧名时让 `integer?` 自己成为该内建实现（Proc 名改为 `integer?`，JS runtime 导出由 `round_$q_` 改为 `integer_$q_`），而不是另起一个 `&` 内部名；语义（有限且恰好没有小数部分）、类型签名、失败方式和 WASM lowering 不变，`round?` 的测试移到 `integer?`。`.integer?` 方法表仍指向 `integer?`。迁移规则 `core-integer-predicate-v1` 只能识别 reader 解析出的 `round?` Proc，旧名删除后无法再工作，随之删除；`core-api-0.28-v1` 剩 1 条规则，`core-api-0.29-v1` 剩 2 条。

`foldl-shortcut` 的公开 Proc 删除，内部的 `&list:foldl-shortcut` 保留（`any?` / `every?` 使用，见 `202610120017-core-shortcut-decoupling.md` 与 `202610150020-private-shortcut-js-abi.md`）。JS runtime 的 `foldl_shortcut` 兼容桥梁一并删除，`check-js-runtime-identity.mjs` 改为断言该导出不存在，私有入口的 early exit、默认值与错误传播检查不变。

残留调用沿用 `retired_core_function_migration`：检查时报告未知名字，告警末尾给出删除版本和改写方式，覆盖裸名、`:refer`、`:as` 与 `calcit.core/` 前缀四种写法（`type_fail.rs` 已有用例表新增两行）。

仍保留：`case-default`（Respo UI 源码与 js-ffi 示例仍在使用）、`%some` / `%none`（Respo、Reel、Recollect 源码）、`atom`（cumulo-reel 源码）、`vals`（Respo 源码）。`%ok` / `%err` 在上述下游中没有调用，但与 `%some` / `%none` 共用 `core-nominal-constructor-v1`，且 core 自身与测试仍有约两百处调用，另行处理。

## English

Related to #2079. 0.31.0 removes the deprecated `round?` and `foldl-shortcut` from `calcit.core`; the ten tracked core libraries have no `.cirru` calls (text scan). `integer?` now owns the built-in implementation directly (Proc renamed to `integer?`, JS export `round_$q_` renamed to `integer_$q_`) with unchanged semantics, signature and WASM lowering; `round?`'s tests moved to `integer?`. `core-integer-predicate-v1` could only match the reader-resolved `round?` Proc, so it is removed; `core-api-0.28-v1` now has 1 rule and `core-api-0.29-v1` has 2. The public `foldl-shortcut` Proc and the JS `foldl_shortcut` bridge are removed while the private `&list:foldl-shortcut` used by `any?` / `every?` stays. Residual calls get the unknown-name warning with the removal release and replacement. `case-default`, `%some` / `%none`, `atom` and `vals` stay because downstream source still calls them; `%ok` / `%err` have no downstream calls but are left for a separate change.
