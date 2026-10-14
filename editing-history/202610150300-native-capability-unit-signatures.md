# 原生异步能力的 Unit 返回证据

## 问题与决策

#2047 在正式 0.29.3 工具链与公开 calcit-http 模块上暴露：普通检查和真实 HTTP 请求、响应、取消均通过，但项目 strict workflow 把内嵌 core 的 `ffi-response:resolve` 返回值视为未知值。下游无法通过编辑自己的 Snapshot 修复内嵌 core。

原因不是 JS FFI body 选择，也不是能力运行时返回值不一致。`&ffi-response-resolve`、`&ffi-response-reject`、`&ffi-task-cancel` 的成功路径原本返回 Unit，但注入注册没有填写已有的 `ProcTypeSignature`。因此用现有签名字段补齐返回证据，不新增公开规则、诊断或 checker。

## 契约边界

公共方法继续保留 FfiResponse/FfiTask 名义接收者及泛型 payload/reason。注册层的 raw capability 和可序列化 payload 是原有开放宿主边界；只标注既有成功返回 Unit，不扩大公共类型、不添加 unsafe、不改变 ABI、参数数量或错误传播。取消仍允许省略 reason；已进入关闭状态的成功取消仍返回 Unit。

## 验证方式

已有 `check-known-assertion.mjs` 用受保护 CLI 事务构造四个可审阅的 Calcit wrapper，分别通过 resolve!、reject!、cancel!、cancel-with! 方法调用验证独立返回证明，而不是改为裸 native call。正式 alpha.2 在该最小用例仍报告 core 的 Unit/unknown 错误；本地候选通过，且无需插入 Unit 尾表达式或 cast。

这部分保留在现有 CLI harness：注入注册归可执行 CLI 所有，不能在通用语言测试中伪造可成功响应的宿主能力。原 definition-attached 能力语义测试及 Rust 的并发、错误和序列化边界测试继续回放；真实 calcit-http 原有 HTTP smoke 验证实际请求响应与取消，没有创建新测试运行器。

候选源码验证与正式发布验收分开记录。工具链配对发布后，受影响消费者仍须用正式 CLI/runtime 完成原 strict、业务和目标后端验收；本修复不等同于 #1916、#2053 或稳定 0.30.0 已完成。
