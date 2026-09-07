# Fix wrapped enum constructor lowering / 修复包装枚举构造降级

## English

- Reuse nominal type-definition source resolution when identifying direct Struct and Enum constructor calls.
- Recognize generic `defenum` values wrapped by `impl-traits`, so direct `MapEntryDecision :keep ...` lowers to the named enum constructor in JavaScript.
- Add focused source-resolution coverage and a native/JS Snapshot regression with an explicit generic `hint-fn` return contract.

## 中文

- 识别直接 Struct 和 Enum 构造调用时，复用具名类型定义的源码解析逻辑。
- 支持由 `impl-traits` 包装的泛型 `defenum`，确保直接 `MapEntryDecision :keep ...` 在 JavaScript 中降级为具名枚举构造。
- 增加源码解析覆盖，以及带显式泛型 `hint-fn` 返回约束的 native/JS Snapshot 回归测试。
