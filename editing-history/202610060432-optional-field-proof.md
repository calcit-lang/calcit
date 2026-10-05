# Optional 字段写入的方向性证明

## 问题与决策

Diary 的既有严格迁移门禁拒绝了已证明的 Number/String 写入对应 Optional 名义字段。无依赖最小程序的普通编译与执行通过，但同一源码的 `fix --workflow strict --verify` 将 `Number → Optional<Number>` 报为需要边界转换。

Optional 是迁移期保留的 nullable 值类型。具体 payload 和 nil 进入对应 Optional 不改变运行值，也不需要额外的 checked decoder；反向消除 Optional 才需要边界证据。此次修复让有界普通类型关系与带泛型绑定的证明遵守同一方向规则，不增加场景规则或检查入口。

对于泛型，先检查实际变量已有绑定，再将目标 nullable 类型投影到 payload：同一个 `T → Optional<T>` 可以证明；未绑定的 T 不能因此证明任意具体类型。绑定仍然只在完整证明成功后提交。

## 保留的边界

- 错误 payload、Dynamic 进入具体 payload、Optional 消除、Optional 与 JsNullish 的跨 wrapper 转换仍需要检查或被拒绝。
- `Ref<Number> → Ref<Optional<Number>>` 仍不安全；引用的双向不变性没有改变。
- 新公共 Fn schema 使用 Option 的既有约束不变；此次修复服务旧名义字段迁移，不鼓励新增 Optional 接口。
- 不改变 Optional 的后端表示、运行时求值、JS truthiness 或 WASM 支持范围。

## 验证方式

语义写在 `calcit/test-struct.cirru` 的五个 definition `:tests`，覆盖直接构造、具体字段写入、0/空字符串、nil 与泛型 payload。既有 `scripts/check-known-assertion.mjs` 回放同一测试 AST 到 native/JS，并在无模块临时项目通过完整 strict workflow。

反例复用该运行器，检查错误 payload、开放输入、host wrapper 转换、Optional 消除、可变引用扩大与泛型不匹配，且拒绝后源码保持字节一致。原有回归与门禁不删减。Snapshot 与临时 fixture 的程序变化通过 revision 守卫的 Calcit 编辑命令完成。

真实 Diary 候选验证保持原业务断言与 CI 门禁。core 本地修复通过只代表未发布候选证据，不能代替正式 CLI 发布与下游完整升级验收。
