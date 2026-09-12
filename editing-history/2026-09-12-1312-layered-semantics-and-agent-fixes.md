# 分层语义与 Agent 可执行修复契约

关联：#997、milestone 0.14.13。

## 本次调整

- 新增分层语义 RFC，明确 Cirru/Snapshot、表层语言、macro 展开后的 typed core、backend 与 host boundary 的职责。
- 区分编译器内部 Unknown/Unresolved、用户显式 Dynamic 与 JsObject/host value；开放值允许安全携带，具体内容使用仍需要类型证据。
- 明确 compiler-owned lowering 不直接写回源码，Agent 自动修复必须具有唯一 source origin、quoted AST replacement 和 revision/fingerprint 前置条件。
- 将共同契约关联到类型路线、Agent 机器协议、用户类型指南、Calx program backend 与内嵌 Agent 指南。
- 约束 0.15.0 的积极迁移：breaking change 先由已发布 0.14.x 工具链提供 fix，并在真实 JS consumer 上验证后再删除旧表层写法。

## 设计知识

- 结构化存储解决可靠查询和修改，不等于 runtime 必须动态，也不等于 macro 展开树应成为新的 source。
- 泛型函数可以安全携带 `Dynamic` 的条件，是它真正参数化且不观察具体内容；需要具体 trait、字段或 primitive 能力时仍应拒绝未证明的关系。
- backend coverage 是共同表层契约的子集。生成成功不能证明运行正确，native/JS 的共同语义需要实际执行；Calx 未支持能力应给出带 source origin 的 structured diagnostic。
- 面向 Agent 的效率来自稳定 typed result、精确 scope、幂等 transaction 和可恢复冲突，而不是更多自然语言提示或质量分类。

## 验证

- `bash scripts/check-docs-md.sh`：69 files、331 blocks 全部通过。
- `git diff --check`：通过。
