# named constructor 规则覆盖 quasiquote 模板

## 依据

#1625 迁移 Respo 时，`respo.core/defeffect` 的模板 `quasiquote $ defn ~effect-name ~args $ %{} schema/Effect ...` 是唯一无法由 `named-struct-constructor-v1` 处理的生产代码：规则遇到 `defmacro` 和 `quasiquote` 整段跳过。模板在每次展开时才编译，定义本身没有编译器 usage trace，原有证明路径无法覆盖。

## 实现

收集与改写共用一个 quasiquote 深度：`quasiquote` 进入深度 1，`~` / `~@` 回到深度 0；`quote`、嵌套 `quasiquote`、非根 `defmacro` 不进入。普通定义迁移深度 0 与 1，`defmacro` 只迁移深度 1，展开期代码保持原样。

深度 1 的构造不依赖 trace，按宏所在 namespace 静态解析原型（复用 `resolve_project_nominal_target`、局部绑定排除和字段完整性检查）。模板中的符号保留宏所在 namespace，展开后直接构造器同样按该 namespace 解析，因此带 namespace 前缀的原型给出 `machine-applicable`；无前缀原型可能被展开处的局部绑定捕获，给出 `requires-review`。`~proto` 原型、`~@` 拼接字段无法静态确认，不给建议。模板内不组合 `do` 整理规则。

没有新增规则编号或诊断编号：改动只扩大现有两条规则的适用范围，`origin_chain.kind` 用 `macro-namespace-resolved-template-constructor` 区分证据来源。

## 验证

- `tests/fix_cli.rs` 新增模板用例：带前缀的 struct / enum 模板自动改写并运行通过，无前缀原型需审阅，`~proto` 与 `quote` 数据不变，重复预览只剩需审阅项。
- Respo main `bff5191` 临时副本：`calcit fix --ns respo.core --def defeffect --rule named-struct-constructor-v1` 给出 1 条 machine-applicable，staged 校验通过；应用后 119 个 Calcit 测试、`calcit js` 与 `test-callback-types` 通过。
