# 2026-08-16 23:41 宏展开字段误报修复

- 修复依赖项目预处理 `defimpl` / `hint-fn` 相关宏展开时，无源码坐标的内部 tag access 被误报为 `W_REQUIRED_STRUCT_FIELD_TYPE`。
- 仅跳过“处于宏调用栈且 receiver 与调用点都没有源码坐标”的生成节点；可定位到用户源码的 required Struct field 错误继续阻断。
- 用 `docs-workflow` 升级分支与 alerts.calcit 0.10.17 做 A/B 回归：该误报从 warning 列表中消失，其余既有 Struct 迁移告警保持不变。
