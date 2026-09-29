# 谓词方法的显式迁移

## 决策

`core-predicate-method-v1` 仅迁移可证明的内建接收者与方法实现：List/String 索引、Map 键/值、Set 成员。旧方法保留原语义，规则不进入已有 preset，也不改变 Struct/Enum 或 `Contains` trait。相同的 `.contains?` 拼写在不同 receiver 上表示不同命题；词法全局替换会改变含义。

机器可应用的建议必须同时满足具体 receiver 类型、旧新方法解析到相同 core definition、相同参数与返回类型、稳定的 source AST 路径。Dynamic 不自动改写，且在缺乏可证明方法来源时可以不产生建议；未知 macro 只产生审阅建议。自定义同名方法与 quoted data 不改。规则仅处理 definition `:code`，attached `:tests` / `:examples` 仍需人工迁移。

Review 发现同一开放 `.contains?` 若同时匹配 List/String/Map/Set 候选，可能在同一个源码路径生成多条重复审阅建议。现在按 definition 和 source path 合并候选：有唯一可证明改写时优先保留可应用建议，否则只留一条不猜测接收者类别的人工审阅提示。当前 `Dynamic` fixture 实际没有可证明的候选，测试明确断言空建议；另用低层 CLI planner 单元测试覆盖多个候选折叠及证明优先级，不把空数组的 `all` 当成保障。

## 验证

Rust CLI 集成测试验证五类迁移、预览、revision 拒绝、应用、幂等、迁移后 attached Calcit 测试，以及自定义方法、开放接收者、未知 macro 和 quoted data 边界。该测试覆盖 source mutation 机制；方法语义本身由 `calcit.core` 的 definition-attached `:tests` 及既有跨后端脚本验证。Respo 当前 Snapshot 的 40 项 definition `:tests` 全部通过；其整项目 fix 预览被已有 `respo.util.format/mute-element` 的 `E_ERASED_GENERIC_RELATION` 挡住，不能把该测试结果说成消费者源码迁移成功。没有修改 Respo Snapshot。
