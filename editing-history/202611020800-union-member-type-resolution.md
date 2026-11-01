# Union 成员类型统一解析 / Unified union member type resolution

## 中文

生态迁移中的真实 union 解码受两个问题影响：成员的短名在调用方 namespace 下解析，且名义泛型成员进入数据形状图时丢失参数。隔离复现中 `Cell Number` 单独解码成功，放进 union 后却报泛型参数缺失；同名嵌套 union 还可能选择调用方的错误定义。

把现有递归类型引用 mapper 原样从 preprocess 移到共享数据模型模块，保持原 public re-export。checked_members 在展平前按每层声明 namespace 统一限定引用；返回完整 annotation，删除临时成员 namespace 字段及集合检查/解码器各自的补丁。数据形状图统一调用既有完整类型构建器，保留 Struct/Enum 泛型实参。没有新增类型状态、proof、诊断编号、改写规则或 checker。

共享 definition tests 覆盖跨 namespace 同名嵌套 union、泛型 Struct 字段和 Enum payload 的正确输入与错误类型；原始 fixture 定义/元数据保留。集合 initializer 和原 decoder 回归继续执行 native/JS，共同跑完整 Cargo、Clippy 和原 check-all。受检数据形状限制及 WASM unsupported 保持，正式 compiler/runtime 发布前不把候选编译器证据冒充正式下游验收。

## English

Real migration repros expose caller-scope resolution of short union member names and dropped generic arguments in nominal union decoder members. Direct `Cell Number` decoding succeeds while decoding the same applied type through a union fails; a shadowed nested union can also select the caller's definition.

Move the existing recursive type-reference mapper verbatim into a shared model module, retaining its preprocessing re-export. Qualify annotations in each union's declaration scope before flattening, returning the complete checked annotation. Remove the temporary member namespace field and client-specific qualification. Reuse the existing full data-shape type builder to retain Struct/Enum arguments. No new proof state, diagnostic, migration rule or checker is introduced.

Definition-attached shared tests cover cross-namespace shadowing, applied generic Struct/Enum decoding, and rejected field/payload types. Preserve original fixture definitions and metadata, retain collection and decoder regressions, and run native/JS, Cargo, Clippy and original check-all. Unsupported shapes and WASM limitations remain; unreleased candidate evidence does not certify a formally published consumer graph.
