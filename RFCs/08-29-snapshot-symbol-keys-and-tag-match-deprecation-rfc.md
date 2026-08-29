# Snapshot Symbol Keys and `tag-match` Deprecation RFC

Status: Implemented / 已实现

## 中文

### 背景

Architecture 文件已经用 Symbol 表示 definition FQN，而 `calcit.cirru` 的 `:files` 与 `:defs` key 仍由 formatter 写成 String。两套持久化表示增加了工具转换成本，也使“标识符”和“普通文本”的语义边界不一致。与此同时，宏 `tag-match` 会在展开后丢失原生 enum 分支结构，不利于穷尽性、payload arity、类型推断及后端优化；原生 `match` 已具备替代条件。

### 决策

1. Snapshot loader 对 namespace/definition key 同时接受 String 与 Symbol。
2. Snapshot formatter 和 writer 只输出 Symbol key，形成“宽读、窄写”的单向迁移。
3. String 与 Symbol 归一化后重名时立即报错，不允许 HashMap 静默覆盖。
4. runtime、format migration 与 detailed snapshot 读取路径共享同一兼容规则。
5. `tag-match` 标记普通 `:deprecated`；`analyze deprecated` 会报告调用，`analyze quality` 会把调用计入 `deprecatedCalls`。新代码及迁移代码使用原生 `match`。

### 兼容性

旧 Snapshot 无需先手工修改即可读取。首次执行 `calcit calcit.cirru edit format` 后会产生标识符 key 的规范化 diff，应单独审阅并提交。新版本写出的 Symbol key 不保证旧 Calcit formatter 可读，因此项目必须先升级工具链再格式化。

## English

### Context

Architecture files already represent definition FQNs as Symbols, while the `calcit.cirru` formatter still writes `:files` and `:defs` keys as Strings. Maintaining two persistent representations adds conversion work and blurs the semantic boundary between identifiers and text. The `tag-match` macro also hides native enum branch structure after expansion, limiting exhaustiveness, payload-arity, type-inference, and backend optimization passes; native `match` now covers its intended use.

### Decision

1. Snapshot loaders accept both String and Symbol namespace/definition keys.
2. Snapshot formatters and writers emit only Symbol keys, providing a wide-read/narrow-write migration.
3. A normalized String/Symbol collision is rejected explicitly instead of being silently overwritten by a HashMap insertion.
4. Runtime, format-migration, and detailed-snapshot readers share the same compatibility rule.
5. `tag-match` receives the ordinary `:deprecated` tag. `analyze deprecated` reports calls and `analyze quality` includes them in `deprecatedCalls`. New and migrated code uses native `match`.

### Compatibility

Legacy Snapshots remain readable without manual edits. The first `calcit calcit.cirru edit format` run produces a canonical identifier-key diff that should be reviewed and committed separately. Because older Calcit formatters are not guaranteed to read newly written Symbol keys, projects must upgrade their toolchain before formatting.
