# 泛型方法调用保留 trait 来源

## 问题

0.28.0 的字段谓词迁移需要确认旧 `Contains` 的语义边界。若一个 Struct 同时实现 `Contains` 与另一个也提供 `.contains?` 的 trait，具体接收者的裸调用会因来源冲突而报错；但 `where T: Contains` 的泛型函数已有唯一来源，不应在运行时再按接收者的方法名选择另一个实现。此前 native/JS 可能误选自定义同名方法，导致检查结果与类型约束不符。#1491 的 Struct 新谓词没有覆盖该交叉场景。

## 处理

预处理在泛型接收者的唯一 trait bound 且具备稳定身份时，将方法调用 lowering 为保留来源的具名 trait 调用。对于来自源码定义的 trait，调用引用原定义，避免重新构造 trait 值而丢失运行时身份；已有运行时 ID 的 trait 可保留该 ID。既无源码引用又无运行时 ID 的占位值不参与这种 lowering。多个同名 trait bound 仍按现有歧义规则报错；具体接收者的裸冲突调用仍要求显式 `&trait-call`。本次不将旧 Struct `.contains?` 批量迁移到 `.contains-field?`，也不扩大 `core-predicate-method-v1` 的自动改写范围。

## 验证

- `test-traits.main/test-qualified-contains-boundary` 的 definition-attached `:tests` 覆盖两个同名 trait 的显式调用，以及泛型 `where T: Contains` 的来源选择；完整 native 与生成 JS trait 测试也会执行此断言。
- 内部 Rust 测试只验证 lowering 引用源 trait 定义而非重建身份。
- 命名检查脚本确认 fix 不自动改写泛型 bound 方法，并运行 native/JS 回归。现有 core WASM 谓词测试继续执行；通用 `&trait-call` 尚未由 WASM backend 支持，因此此 trait 冲突用例暂不宣称 WASM 可运行。
