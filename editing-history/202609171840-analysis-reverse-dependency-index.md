# 增量分析的反向依赖索引

- 在 definition-local 分析缓存中保存 compiler-resolved direct dependency index，复用 definition 与 namespace revision。
- namespace 的 `ns` 表达式变化会重新解析该 namespace 的定义，避免 import 改动沿用旧依赖边。
- 依赖边包含 macro 展开时 resolver 实际看到的定义引用，并补充 schema 中限定的类型与 trait 引用。
- 每次变化同时基于旧图和新图计算反向传递 affected 集合，删除或改写依赖边时仍能找到原调用者。
- 当前索引只输出失效证据，不缓存或跳过严格预处理；后续必须先验证 schema/import/type-slot 的失效集合，再接入 compiled-definition cache。
