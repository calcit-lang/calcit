# 0.28 核心 API 升级集合

关联 #1458。新增冻结的 `core-api-0.28-v1`，组合 15 条已实现的谓词、标量转换和方法别名规则。集合只替换已证明的叶子；结构改写仍使用现有独立规则，以保持源码路径与预览 fingerprint 的含义。

组合回归发现旧上下文检查把所有普通函数调用也按未知宏处理。现在复用 reader 的 Proc/Method 结果及 compiler-resolved definition usages，区分求值参数和可能观察源码拼写的宏；不按普通函数名建立第二张白名单。未知宏、开放接收者、自定义同名方法和 quoted data 仍按原规则审阅。

预览增加源区域说明：扫描 definition code，attached tests/examples 要人工核对。旧 surface preset 和 strict workflow 的冻结范围不变。谓词规则用源码候选或已解析的 compiled dependencies 先筛选，避免重新追踪无关的 type-slot 声明，同时保留 alias 引用。

验证包含嵌套叶子组合、revision 前置条件、重复为空及 Calcit 定义测试。真实 Respo Snapshot 副本自动迁移 15 处后，重复预览没有 machine-applicable 建议，仍保留 32 处 requires-review；严格检查及 48 个 attached tests 通过。生成 JS 用本分支匹配的 unreleased runtime 验证，现有 DOM/SSR host 测试通过；临时 loader 只选择匹配 runtime，不改变项目源码或生成结果。不能把自动部分幂等解释为整个消费者迁移完成。
