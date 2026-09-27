# Option 内部 helper 的首批方法迁移

## 背景与选择

#1426 已确认公开源码优先让 `Option<T>` 接收者调用 `.unwrap` / `.unwrap-or`，而不是把 `option:unwrap` / `option:unwrap-or` 作为平行公共 API。#1427 的首批实现只处理这两个高频内部 helper，不一次性改名所有 core 函数，也不新增顶层检查命令。`calcit fix --rule core-option-method-v1` 复用既有预览、revision、源码 fingerprint 与 staged validation 流程；已发布的 `surface-latest-v1/v2` preset 不被悄然改变。

## 自动改写边界

只有编译器把调用头解析到相应的 `calcit.core` 定义、调用 arity 正确、源码位置可回溯，且接收者的预处理静态类型能证明 `.unwrap` / `.unwrap-or` 的实际实现指向同一个 helper 时才应用。改写把原接收者放在第一位，方法名放在第二位，其余参数保持顺序；每个表达式仍各求值一次。类型为 `Option<Dynamic>`、helper 当函数值、未知 macro 展开或缺少静态证据时只报告 `requires-review`，不插入 `Dynamic`、`unsafe-coerce` 或默认值。

方法分派契约由编译器现有 `static_method_contract` 提供，不维护第二份 Option 签名表。旧 helper 仍作为 core 方法实现存在；本次只收拢应用源码的首选写法，待真实消费者逐步迁移后再讨论移除条件。规则暂不遍历 definition-attached `:tests` / `:examples`；它们需使用 Calcit 测试和人工 review 验证。

## 验证

CLI 集成测试覆盖有类型接收者的预览、apply、revision 拒绝、幂等、应用后的 Calcit `:tests`，以及开放类型只给人工建议。文档示例经 `calcit docs check-md` 检查。实际消费者只对能够取得完整来源与静态方法证据的调用自动迁移；遇到宏边界或旧项目固定较早 CLI 版本时，不用兼容模式强行改写。
