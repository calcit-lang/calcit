# 方法字面量展示

- `Calcit::Method` 的展示与 `turn_string` 统一输出 Calcit 可写的字面量语法，例如 `.ceil`、`.display-by`。
- `defimpl` 与 `deftrait` 过去从 `format-to-lisp` 的 `(&invoke name)` 内部格式中截取方法名；展示改为 `.name` 后，需改为去掉首个点号并保留其余字符串。
- 验证应覆盖 Rust 测试、JS 编译运行，以及 `cr eval '&methods-of 1'` 的实际输出，避免 trait/impl 方法名在预处理时退化为 tuple 方法调用。
