# 开放集合 fold 的局部参数证据

## 问题与决策

显式 `Dynamic` 集合配合 `Fn(Dynamic)->Bool` predicate 的短路 fold，运行时可返回 Bool，严格返回证明却把整个结果退回 Dynamic。集合元素开放并不意味着独立的 accumulator 也必须开放。

修复共享类型路径：函数参数作用域保留显式 Dynamic；已编译函数的实现推导重建参数作用域；fold callback 上下文和普通 fold 返回推导共用既有 specialization。缺失作用域仍是缺证据，不默认补成 Dynamic，也不信任函数返回声明。

参数的显式 Dynamic 证据在既有 proof audit policy 下保留，不借此提前翻转普通入口的全面收紧。真实 Respo 回放发现自定义 `component?` 与 `and` 守卫尚未向 nominal consumer 传递证明；未受控扩大 ordinary policy 会使 main/reload 和 8 个 unit 用例失败。#1540 已明确消费者迁移是 #1538 后续默认 rollout 的门禁，0.29 交付显式证明工具，因此此处沿用既有 policy flag，不把未证明 predicate 当成类型转换，也不修改用户未提交的 Respo Snapshot。

短路 fold 仍逐支证明 Bool 控制槽与实际 payload，默认值也必须满足 accumulator 类型。普通 fold 仍要求 reducer 满足输入和返回契约。开放元素不能获得具体 payload 证明；已知非集合 receiver 不进入 specialization。

首次完整 core 回归暴露 `assoc-in` / `update-in` 的分支合并退化：根级 Dynamic 的计分为 200，而两个开放槽的 Map 计分为 400，兼容性启发式错误保留 Map 形状。根级 Dynamic 必须吸收分支合并，不能通过动态槽数量为未知形状授予 Map 证明。继续保留原有 Calcit 嵌套 Map/List/nil 路径测试，并扩展既有低层 annotation 合并测试覆盖两个分支顺序。

## 验证契约

`foldl-shortcut :tests` 的 `open-fold-proof` 用例覆盖带显式 predicate 的 Bool、空集合默认值和普通 fold。既有跨后端 runner 回放 native/JS，并用显式 concrete-return-proof-v1 审计，不为消除 generic equality 的独立证明问题删除运行时断言。错误 payload、开放 member、错误默认值与已知非集合 receiver 必须拒绝，source 字节不变。

此改动不自动解决 bare Fn callback、WASM 开放 receiver 或完整 Diary 迁移；这些仍保留各自证据义务。最终验证记录放在 PR 中。
