# 集合方法查询使用公开 schema

## 问题

#1455 的基线中，`query type ":: 'List 'Number"` 把 `.append` 标成 `open`，但 `calcit.core/append` 的 Snapshot schema 已明确声明 `List<T>, T -> List<T>`。方法表把它解析为内建过程；查询逻辑取运行时较宽的 `List, Dynamic -> List` 签名，丢失了公开的泛型关系。`.len` 已由 Len trait 提供精确契约。

## 决策与边界

- 查询方法合约时，内建过程优先读取已加载 definition 的 `Fn` schema；没有公开函数 schema 时仍使用内建过程签名。
- `.append` 现在对已知的 `List<Number>` 提供 `Number -> List<Number>` 证据。`List<Dynamic>` 仍保持开放；不凭查询结果扩大动态输入许可。
- List `.add` 目前仍是单元素追加的兼容入口，底层 Add trait 的 List 组合另有同名方法。此批保留两者；迁移规则必须先证明 receiver 与来源，避免旧 `.add` 在退出兼容窗时意外转成组合语义。
- `.len` 是 List/Map/Set/String 长度的首选，String 使用 Unicode 标量；Enum/Struct 的 `.count` 暂不归入容器长度迁移。

## 验证

Calcit definition `:tests` 使用真实方法调用验证 `.append/.add/.concat/.len`；Rust 测试仅覆盖查询证据的内部边界。Agent interface 检查结构化查询的精确签名，文档示例执行 `docs check-md`。
