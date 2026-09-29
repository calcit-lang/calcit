# Number 整数谓词方法的同实现迁移

## 决策

`round?` 与 `integer?` 的公开函数仍分别存在，旧名保持兼容。Number 的 `.round?` 与 `.integer?` 现在都指向 `calcit.core/integer?`；该函数仅将 Number 实参传给已跨目标验证的 `round?` 内建 Proc。因此两个方法有同一运行实现、相同的 `Number -> Bool` 静态契约，以及相同的求值与失败路径。既有数值 definition `:tests` 同时覆盖旧新方法、有限整数、非零小数、无穷与单次求值，并在 native、生成 JS、core WASM、WASI Component 运行。

复用已有方法别名 fix 的 receiver 类型、静态方法解析、同实现同契约、源码上下文、revision 与 staged validation 证据，纳入既有 `core-integer-predicate-v1`，不另增一条公开规则。只自动改写可证明为 Number 的 `.round?` 调用；自定义同名方法不提示，开放接收者与未知宏不自动改写。definition 附带 `:tests` / `:examples` 仍是人工检查边界。

## 验证

Calcit definition 测试跨四目标复跑；CLI 回归覆盖 Number 字面量和带 schema 的参数、旧新 query method definition、宏审阅、自定义同名方法不改、preview/apply/revision/幂等与现有函数迁移共存。
