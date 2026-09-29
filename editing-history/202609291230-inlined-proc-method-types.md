# 内建方法 lowering 后的参数类型检查

## 发现

严格模式下，`&map:assoc ({} (:a 1)) :b |oops` 正确报告 `W_PROC_ARG_TYPE_MISMATCH`，但同一接收者的 `m .assoc :b |oops` 原先通过检查并被推断为 `Map<Tag, Number>`。方法实现已选中内建 Proc，却在返回内联调用前跳过了 Proc 参数检查；继续只依赖生成代码的下一轮预处理不能保证诊断执行。

## 决策与边界

在通用方法参数检查路径，对已解析为 Proc 的方法直接复用现有 `check_proc_arg_types`，使方法写法和前缀调用遵循同一签名，不新增 Map 专属规则或改变运行时效果。严格 CLI 回归同时覆盖正确值通过和错误值拒绝。

Map `.add` 仍调用 `&map:add-entry`，其 `List<P>` entry schema 无法证明异构 key/value 分别满足 `K/V`；本改动**不**声称修复该旧入口。#1479 后续应决定其退场或显式开放边界，不能把 `.add` 当作 `.assoc` 的类型安全别名。其他用户自定义方法和未证明的动态调用也不据此收窄。
