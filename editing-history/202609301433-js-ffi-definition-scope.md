# 被引用定义的 JS FFI 权限隔离

## 背景

Diary 的 `app.client/*states` 在顶层 `defatom` 初始化中使用了 `unsafe-coerce`，自身没有 `:js-ffi` 权限，却被带有该权限的入口函数间接引用。此前 `--check-only` 错误地通过：按需预处理被引用定义时，编译器沿用了调用方的临时权限。

## 决策

按需预处理源定义前清除调用方的函数权限；顶层值不能继承引用者的权限。若源定义的代码本身是 `fn` 且声明了函数 schema，则把 schema 作为该顶层 `fn` 的预期类型，使其参数类型及 `:js-ffi` 权限来自自己的契约。命名 `defn` 已会自行读取 schema，不能让其预期类型误传给内部回调。宏展开生成的代码仍按既有规则继承其词法调用环境，不把宏当成独立的源定义。

此修复不把 `Dynamic` 全部禁用，也不自动插入 `unsafe-coerce`。合法的 FFI 应留在显式标记的适配函数里；Diary 的状态表示需要另行迁移为可验证的类型。

## 验证

- `unsafe-coerce-global-leak-strict.cirru`：标记的调用者不能授权未标记的顶层 `defatom`。
- 原有 `unsafe-coerce-scoped-strict.cirru`：标记的适配函数继续可用。
- 全量集成检查覆盖宏生成代码及 JS FFI 测试，防止作用域隔离误伤合法展开。
