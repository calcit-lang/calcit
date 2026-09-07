# Strict diagnostics by default / 默认启用严格诊断

## 中文

- Calcit 0.14 默认启用严格预处理诊断，并对缺省的 JS FFI policy 使用内存中的 `:error` 默认值。
- `--strict-types` 保留为显式的零类型债务 quality gate；新增临时迁移开关 `--compat-types`，且两个开关互斥。
- 内部历史测试 fixture 显式使用兼容模式，避免把旧语义覆盖误认为 0.14 新项目的推荐配置。
- `eval` / `exec` 的合成入口使用结构化的零参数 `Fn` schema，并只在返回位置保留显式 `Dynamic`，使默认严格诊断仍可检查临时代码。
- 新增二进制级 smoke，覆盖默认严格成功/失败、兼容逃生口、互斥参数以及 `eval`，并接入测试与发布 workflow。
- 更新 CLI、升级、JS interop 与 nil 迁移文档，说明 0.13.79 到 0.14 的迁移路径。

## English

- Calcit 0.14 enables strict preprocessing diagnostics by default and applies an in-memory `:error` default when the selected entry omits a JS FFI policy.
- `--strict-types` remains the explicit zero-type-debt quality gate; the temporary `--compat-types` migration switch is mutually exclusive with it.
- Internal legacy fixtures opt into compatibility mode explicitly so their historical coverage is not presented as the recommended 0.14 project configuration.
- Synthetic `eval` / `exec` entry points use a structured zero-argument `Fn` schema with an explicit open return, so default strict diagnostics can still validate ad-hoc code.
- A binary-level smoke covers default-strict success/failure, the compatibility escape hatch, conflicting flags, and `eval`, and runs in both test and publish workflows.
- CLI, upgrade, JS interop, and nil-migration documentation now describes the 0.13.79-to-0.14 migration path.
