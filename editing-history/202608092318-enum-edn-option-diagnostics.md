# Cirru EDN enum identity and Option migration diagnostics

## 背景

- GitHub #326 暴露 JS `format-cirru-edn` / `parse-cirru-edn` 往返后 enum variant 从 interned `CalcitTag` 变成 `CalcitSymbol`，导致生成的 `match` 按身份比较时无法命中。
- GitHub #325 暴露 Option migration 诊断仍使用预处理前的 source symbol，普通 `=`, `some?`, `assoc` 等调用不会触发告警，并且 `starts-with?` / `ends-with?` 的 proc metadata 比公开 schema 更宽。

## 修改与边界

- JS Cirru EDN parser 在匿名和具名 enum 分支统一把解析出的 symbol variant 通过 `newTag` intern；quoted nominal name 也按 tag key 恢复 options map 中的 `CalcitEnumDef`。
- Option migration diagnostics 改用预处理后的 `head_form` / `call_head`，避免漏掉普通 source syntax，同时继续区分应用自行定义的同名函数。
- 结构操作诊断覆盖 `assoc`, `dissoc`, `merge`, `update` 及其 nested/non-nil variants。
- `starts-with?` / `ends-with?` proc 参数 metadata 收紧为 `String × String`，与文档和 core schema 一致；旧 Tag runtime 兼容测试用显式 `unsafe-coerce` 标明边界。
- 合法的 nominal enum equality 继续允许，包括类型推断尚未完成时，Option/Result constructor 和已声明返回 nominal enum 的调用之间的比较。

## 验证

- JS runtime identity check 覆盖匿名 enum、具名 enum prototype 恢复和 identity-based match 条件。
- Rust end-to-end snippet test 覆盖 issue 中的 `get-env`, `some?`, `update-in/assoc`, `nth/starts-with?` 普通源码写法。
- `cargo fmt --all`, `cargo clippy -- -D warnings`, `yarn compile`, `cargo test`, `yarn check-all` 全部通过。
- 安装当前全局 `cr` 后在 Respo 执行 `cr --check-only`，成功在真实项目中报告 3 个 Option migration warning（另有 1 个既有 JS nullable warning），未进入相关运行时失败路径。
