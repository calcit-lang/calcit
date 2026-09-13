# `edit def` 创建 macro 时同步生成严格 schema

## 背景

严格 macro loader 不接受 whole-Dynamic schema，但 `edit def` 过去对所有新 definition 都使用
`CodeEntry::from_code` 的 Dynamic 默认值。命令会先报告成功，后续任何 query/edit 再因 Snapshot
不可加载而失败，Agent 无法用下一条结构化命令修复。

## 修改

- 把 formatter 迁移 direct-quote macro 时使用的参数形状恢复逻辑提取为共享 helper。
- `edit def` 在写盘前识别 `defmacro`，生成 Syntax required/optional/rest、`Expr<Dynamic>` expansion
  和空 capabilities 的保守严格 contract。
- 新建 macro 直接使用该 contract；从非 Macro definition 覆盖为 macro 时替换不兼容 schema；已有
  严格 Macro schema 继续保留，避免覆盖人工收窄结果。
- 参数列表缺失或 required/optional/rest 形状非法时，在加载和写盘前拒绝。

## 验证知识

- 普通 `edit def`、transaction 子进程与 `edit format` 必须复用同一 schema 构造函数，不能维护三套规则。
- required/rest macro 的展开语义用严格模式下的 definition `:tests` 验证。
- `?` 参数本身属于 legacy optional contract，默认严格模式会报告 `E_LEGACY_OPTIONAL_PARAM`；仅其兼容
  展开测试显式使用 `--compat-types`，不能因此放松新建 Snapshot 或 loader 的默认门禁。
- Rust 集成测试只负责 CLI 子进程、schema 序列化、transaction 一致性及失败时文件内容不变。
