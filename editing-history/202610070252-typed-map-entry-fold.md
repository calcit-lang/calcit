# Map 键值迭代保留类型关系

## 问题与取舍

Diary 的严格升级在 `filter-map-kv` 与 `.filter-kv` 的内部回调被阻断。
现有公开 schema 已表达 `Map<K,V>`，但实现先形成异质的二项 List，
同质 List 的元素类型只能是 Dynamic，随后读取槽位丢失独立的 K/V 证据。
`map-list-kv` 的独立 callable proof 同样能复现；`map-entries` 依赖它。

使用一个内部 `&map:fold-kv`，从原始 Map 迭代器把 accumulator、key、value
分别传给回调。它声明 `Map<K,V>, A, Fn(A,K,V)->A -> A`，复用已有 fold
的上下文推断和返回证明，不增加诊断编号、审计入口、迁移规则或用户侧替代 API。
空集合 accumulator 仅采用回调输入中的已有类型证据，不能用回调输出反向证明输入。

## 兼容边界

公开签名、原有附带测试与例子不变；原始迭代器顺序、每项调用一次、空 Map
不调用回调及错误传播保持原语义。旧 pair fold 与 `.to-list` 的异质 List
边界不变，显式 Dynamic 不会被具体回调参数消除。
外层泛型由内部回调捕获，而不是重新声明成不相关的同名参数。

## 验证方式

在三个公开定义的 `:tests` 加入名义字段、泛型捕获、求值次数、迭代、键冲突与失败行为，
复用已有 native 与跨 namespace 相同 AST 的 JS 回放。严格负例覆盖错误的键、值、输出、
MapEntryDecision、谓词返回值以及开放 payload；JS 拒绝时不得生成应用代码。
Rust 测试只检查现有 callable/return proof CLI 输出与源文件不被修改。
提交前执行仓库 cargo test、clippy 和 yarn check-all（包括原有 WASM 门禁）。
问题与交付状态由 #1793、#1529、#1553 及关联 PR 记录，不将候选代码当作已发布版本。
