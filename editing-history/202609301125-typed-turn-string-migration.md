# 对旧 `turn-string` 的受控迁移

## 决策

沿用已有的 `core-identity-conversion-v1`，不新增 fix 规则或命令入口。仅在 reader 确认是内建 `turn-string`、参数静态类型可证明为 Nil、Bool、Number、String、Tag、Symbol 之一，且调用的源码位置稳定、不跨未知 macro 时，把调用头改为 `calcit.core/to-string`。这些类型的内建 `ToString` 实现都转调同一 `turn-string` primitive，因此实参只求值一次，文本和失败行为保持不变。

`Dynamic` 与未知类型只给人工审阅；已知集合不产生迁移建议。自定义名义类型虽可能实现 `ToString`，旧 primitive 并不据此承诺可转换，因此不能把任意泛型或 trait-bound 输入自动改成方法调用。Respo 属性和 HTML 消费者仍有合法的开放 `Dynamic` 值，不作为批量迁移对象。

## 验证

临时 Snapshot 的 Calcit `:tests` 对迁移前后的 String、Number、Tag、Bool、Nil 文本进行断言；fixture 同时验证 Dynamic 只需人工审阅、List 不改写、过期 revision 拒绝、二次应用无变更。用 Respo `respo.render.html` 只读预览检验开放属性值没有 machine-applicable 建议，运行其定义内测试确认真实消费者行为未受影响。其余仓库质量门禁与 PR 最新 HEAD CI 分别核对。
