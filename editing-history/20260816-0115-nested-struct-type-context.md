# 嵌套 Struct 类型的声明上下文与迁移告警边界

## 背景

issue #357 在 cumulo-reel 的完整依赖图中暴露出 `W_REQUIRED_STRUCT_FIELD_TYPE` 误报：`ClientStore` 的 `:router` 字段在声明处写作同 namespace 的短类型 `'Router`，字段类型流入调用方后只剩未限定 `TypeRef("Router")`，无法再次解析为 `cumulo-reel.schema/Router`。正确的 `:name router` 因而失败，促使调用方用 `&struct:get` 绕过检查。

## 修改要点

- 从具名 Struct 读取字段类型时，按 Struct 定义所在 namespace 解析字段类型体内的短 `TypeRef`，并递归处理泛型参数、集合、函数、Optional 等复合类型。
- 函数参数 schema 进入函数体时，同样先解析词法局部类型，再解析声明 namespace、import alias/refer 与 core 类型；带源码引号的局部引用会在 lookup 前规范化。
- `&struct:get` 的迁移告警只针对项目源码中的 literal tag 字段访问；普通执行、query/static-analysis 与 docs snippet 入口都保留项目 namespace 集合。加载依赖、动态 key、core/runtime 与可复用 `defimpl` 保留低层边界，避免旧依赖阻塞项目迁移。
- 未解析 nominal receiver 的诊断会明确要求恢复或限定 schema，而不是建议用 raw accessor 隐藏问题。
- `/tmp` 提示区分主 Snapshot 与 `--file` scratch 输入：Snapshot 应留在项目根目录保证相对模块解析，`.calcit/snippets/` 只用于项目本地临时代码。

## 回归证据

- 单测覆盖同 namespace 嵌套 Struct 字段、带引号局部 TypeRef、未解析 nominal 诊断、动态 key 与依赖 namespace 告警边界。
- 在 cumulo-reel issue 提交者对应提交的副本中，把 Router/Session 的七处 raw 访问恢复为 tag 语法；`query type-at` 得到精确的 `'cumulo-reel.schema/Router`，项目自身不再出现相关 required-field 误报。
- `cargo clippy -- -D warnings`、`cargo test`、`yarn compile`、`yarn check-agent-interface`、`yarn check-all` 与相关 Markdown snippet 检查通过。
