# 公共函数 schema 以外的 `'Optional` 改为建议级提示

## 证据与范围

0.32.0 删除 `Optional<T>`（#2233）。在此之前只有公共函数 schema 会得到
`W_LEGACY_OPTIONAL_SCHEMA` / `E_LEGACY_OPTIONAL_SCHEMA`；`defstruct` 字段、`defenum`
payload、`def` 值的 `:schema`、局部 `hint-fn`、`assert-type` 与解码目标中的 `'Optional`
没有任何诊断，删除后会直接失去解析，升级时没有预警（#1554）。

0.31.0 只做不破坏的准备：新增建议级 `W_LEGACY_OPTIONAL_TYPE`，不让任何检查失败，
不新增语法，也不提供自动改写。

## 设计

- 复用 `contains_legacy_optional` 判断类型中是否含 Calcit 来源的 Nullable。
- 走 `push_advisory`，只出现在 `--check-only` 的 `Notes:`，不进入 warning 列表，
  因此严格模式、`--compat-types` 与运行都不会因此失败。
- 新编号而非复用 `W_LEGACY_OPTIONAL_SCHEMA`：后者是会让 `--check-only` 失败的 warning，
  严格模式下升级为 error；同一编号同时表示阻塞与不阻塞会让按编号过滤的工具误判。
- 只报告项目源码（`should_emit_project_source_lint`），并要求诊断位置在当前 namespace
  且不是生成代码；编译器从定义 schema 注入的 `hint-fn` 没有源码位置，不会重复报告。
  core 的 `&` 原语和 `nil->option` 保留与公共 schema 检查相同的内部例外。
- 结构体字段和枚举变体指向各自的 `(:tag ...)` 源码位置；同一变体多个 payload 只报一次。

## 限制

- Notes 只覆盖从 init / reload 入口可达的定义；`--keep-going` / `--all-defs` 的逐定义
  报告不包含建议级诊断。

## 验证方式

`tests/fixtures/legacy-optional-advisory.cirru` 在严格模式下覆盖六个位置各报一次、
`Option<T>` 写法不报告且检查通过（`cr_tests::type_fail`）；core 例外由
`preprocess` 单元测试固定。
