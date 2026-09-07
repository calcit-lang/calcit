# Fix generated-function macro provenance / 修复生成函数宏来源判断

## English

- Followed up the post-merge review on #903 by defining macro provenance from the newest call-stack frame only.
- Reused one helper in both generated-function schema synthesis and strict public-schema rejection so the two decisions cannot drift.
- Added a strict regression where a macro expansion reaches an independently authored definition without a schema. The definition must still fail with `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA`, while the existing generated-closure regression remains accepted.

## 中文

- 跟进 #903 合并后的审查意见，仅使用调用栈最新 frame 判断宏生成来源。
- 生成函数 schema 合成和严格公开 schema 拒绝共用同一 helper，避免两处判断漂移。
- 新增严格模式回归：宏展开间接访问一个没有 schema 的独立源码定义时，该定义仍必须以 `E_WHOLE_DYNAMIC_PUBLIC_SCHEMA` 失败；已有的真实宏生成闭包仍保持通过。
