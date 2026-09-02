# Stable nominal callables and typed literal-path updates

2026-09-02 16:04 CST

## English

- Promoted capture-free anonymous nominal impl methods to deterministic compiler-owned callables such as `&lagopus0:show`, while preserving lowercase `.show` in application source.
- Kept captured impl closures on runtime dispatch, converted existing named method functions to direct imports, and added namespace qualification only when cross-namespace nominal names could collide.
- Recorded the source `defimpl` as a compiled dependency so changing an implementation invalidates its generated callable and direct callers. Source and generated-symbol collisions report stable typed diagnostics instead of silently overwriting definitions.
- Extended typed literal-path lowering from `get-in` and `assoc-in` to `update-in`. Fully typed nested Maps with non-empty literal paths now use direct Option-aware lookup and reconstruction, with the receiver, path expressions, updater, and replacement each evaluated once in source order.
- Preserved dynamic paths, Dynamic receivers, mixed containers, and captured closures as explicit compatibility boundaries rather than guessing static semantics.
- Allowed a named `impl-traits` wrapper type to match the concrete Struct/Enum value it resolves to, so hooks can expose precise nominal plugin return types without losing method lookup identity.
- Preserved legacy `?` optional-parameter evidence in the cached callable shape and method checker, preventing valid calls such as `.show d!` from being rejected when the final text argument is omitted.
- Hardened generated-callable invalidation after review: canonical impl tags win over aliases, inline `impl-traits` values depend on their nominal source definition, and synthesis falls back to runtime dispatch when no reliable source owner exists. Reprocessed Dynamic parameters also clear shadowed outer type evidence.
- Added Rust unit coverage, generated-JS assertions, native runtime checks, documentation, and cross-backend regression coverage.

## 中文

- 将不捕获词法环境的匿名 nominal impl 方法提升为确定的编译器内部 callable，例如 `&lagopus0:show`；应用源码仍然只写小写 `.show`。
- 捕获闭包继续使用运行时分发；已有命名方法函数改为直接 Import；只有跨命名空间同名 nominal 类型存在冲突风险时，生成 scope 才自动带命名空间。
- 将来源 `defimpl` 记录为编译依赖，使实现变更能够使生成 callable 和直接调用者失效重编译。与源码定义或其他生成符号冲突时返回稳定的类型错误码，不再静默覆盖。
- 将类型化字面量路径优化从 `get-in`、`assoc-in` 扩展到 `update-in`。完整类型的嵌套 Map 与非空字面量路径会生成 Option 感知的直接读取和重建链，并保证接收者、路径表达式、updater 与 replacement 按源码顺序各求值一次。
- 动态路径、Dynamic 接收者、混合容器和捕获闭包继续作为显式兼容边界，不猜测其静态语义。
- 允许命名的 `impl-traits` 包装类型与其解析得到的具体 Struct/Enum 值匹配，使 hook 可以声明精确的 nominal plugin 返回类型，同时保留方法查找身份。
- 在缓存的 callable shape 与方法检查器中保留旧式 `?` 可选参数证据，避免 `.show d!` 这类省略末尾文本参数的合法调用被误报。
- 根据 review 加固生成 callable 的失效关系：canonical impl tag 优先于别名，inline `impl-traits` 依赖其 nominal 源定义；找不到可靠源码 owner 时回退到运行时分发。重新处理 Dynamic 参数时也会清除被遮蔽的外层类型证据。
- 补充 Rust 单元测试、生成 JS 断言、native 运行验证、文档与跨后端回归门禁。
