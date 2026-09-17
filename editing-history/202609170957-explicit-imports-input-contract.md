# 明确批量 imports 的输入契约

## 问题

`edit imports` 仍通过输入首字符猜测 Cirru 与 JSON，和其他 syntax mutation 已有的 `--input-format` 契约不一致；JSON 的“规则数组”也无法明确表达它承载的是完整语法节点还是命令专用数据。

## 调整

- `edit imports` 支持 `--input-format cirru|json-ast|auto`。
- 显式格式统一承载一个以 `[]` 开头的完整 imports 语法节点；Cirru 通过 `quote` 越过代码/数据边界，JSON AST 只使用字符串和数组。
- `auto` 原样保留旧 JSON 规则数组读取能力，避免破坏已有脚本；新文档与 Agent 指南不再推荐隐式识别。
- 单元与 CLI 测试覆盖两种显式 transport、旧兼容输入和错误 envelope，并固定 syntax AST 与 FFI EDN map 的诊断边界。
