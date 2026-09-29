# 索引、键和值谓词的首批明确名称

## 范围

List/String 的 `.contains-index?` 分别复用 `&list:contains?` / `&str:contains?`；Map 的 `.contains-key?` 与 `.contains-value?` 分别复用 `&map:contains?` / `&map:includes?`。旧 `.contains?/.includes?` 保留原义。即使 Map 的 K/V 类型相同，两种新名字也依靠不同的实现来源区分命题。Set 成员继续用 `.includes?`；Struct/Enum 和自定义 `Contains` trait 尚未迁移。

## 跨目标边界

共享的 Calcit definition `:tests` 覆盖 List 索引和成员区分、负数/小数/非有限索引、空 List、Unicode String 索引与子串、Map 同型 K/V 和异型 K/V、空 Map。新脚本从 definition 读取同一测试，在 native、生成 JS、core WASM 和 WASI 0.3 Component 执行。

测试揭示原有 `&list:contains?` 的 JS 实现把范围内小数当作有效索引，而 WASM 将负数直接转换为无符号整数时可能陷阱。JS 现在要求整数；WASM 比较浮点索引的非负、界内和整数性，避免先转换再判断。String 的负数/小数仍按既有行为报错，越界返回 false；不把两种容器的错误策略混为一谈。

本批仅交付方法和契约，不把 `Contains` trait 或用户自定义同名方法批量改写；安全的显式 fix、Struct/Enum 边界和真实消费者迁移仍由 #1482 后续推进。另补 `round?` 旧定义的查询文档，收尾 #1481 的可发现性验收。

Respo 当前 `calcit.cirru` 的 `contains?` 实际调用面向 Set；以本分支编译的 Calcit 跑其 definition `:tests`，40/40 通过。该回归不等于已完成消费者源码迁移，后续仍需在 fix 具备可证明条件后决定是否改写。
