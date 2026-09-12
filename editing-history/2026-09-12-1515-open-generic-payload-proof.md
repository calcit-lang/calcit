# 开放泛型 payload 的方向性证明

## 背景

此前严格模式通过统计泛型变量在签名中的出现次数，再递归寻找 Dynamic 位置来判断“泛型关系被擦除”。这会把 `%some Dynamic`、`Option<Dynamic>`、`Struct<Dynamic>` 等只负责保存和传递开放 payload 的操作也判为错误，并为 `get` 维护额外的来源追踪逻辑。

## 调整

- 类型 proof 明确区分安全拓宽与不安全收窄：具体类型可以证明为 Dynamic，Dynamic 不能在没有边界证据时证明为具体类型。
- 当期望位置是类型变量时，Dynamic 被精确绑定为开放的泛型参数；后续参数不会把该变量偷偷收窄，泛型返回值也继续保持 Dynamic。
- 严格泛型门禁直接复用 proof 与 bindings，只在调用要求把开放绑定收窄或依赖未知 callable 时失败；删除泛型出现次数统计、重复的类型树遍历和 `get` 专属 Dynamic provenance。
- 保留 `E_ERASED_GENERIC_RELATION` 错误码兼容，诊断文字改为说明具体的开放绑定收窄。

## 测试

- 在 `get`、`get-in`、`nth`、`option:unwrap-or`、`option:fold`、`if-let` 的 definition `:tests` 中，以 Calcit 表达开放 Map/List/Option 的读取与消费语义。
- Rust 仅保留 proof 方向、泛型绑定与 callback 逆变的低层不变量；纯传递 `Dynamic` 可以通过，只接受 Number 的 callback 仍不能消费开放 payload。
- 独立 strict `calcit exec` 组合验证覆盖开放读取、默认值、fold 与 if-let，不依赖兼容模式放过诊断。
- `calcit/test-traits.cirru` 增加同一组 Calcit 表达式的 native/JavaScript smoke，并把需要 Number 方法的旧 Option/Result 测试改为显式 concrete payload，避免依赖 Dynamic 被 fallback 偷偷收窄。
