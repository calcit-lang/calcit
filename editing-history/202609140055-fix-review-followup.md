# 修正 #1077 review 反馈

- transaction 文档使用实际 EDN key `:original-revision`，避免 apply 阶段绑定错误字段。
- human fix preview 同时识别 quoted AST 与 redundant-do 的 splice envelope，使 After 段落始终显示 fenced Cirru 源码。
