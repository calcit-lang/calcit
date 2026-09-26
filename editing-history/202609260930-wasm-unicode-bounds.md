# WASM 字符串索引与边界对齐

关联：#1381，0.23.0 milestone。此修改修复现有 WASM lowering，不扩展 WASI 宿主能力。

## 取舍与边界

`nth`、`first`、`rest`、`slice`、`contains?` 的字符串索引使用 Unicode scalar，而不是 UTF-8 字节位置。复用有边界检查的 scalar 扫描和区间复制，避免截断多字节字符；空串、反向区间与越界索引不再触发长度下溢或读取其他内存。`byte-count` 保持字节语义，`count` 的既有 scalar 语义不变。

非法索引（负数、小数、NaN、无穷大）在 WASM 中 trap；超大但合法的非负整数安全饱和后按越界处理。先按源码顺序求值所有实参，再验证索引，避免错误路径丢失末尾参数的副作用。

普通 `slice` 按已有静态 String/List 类型证据选择 lowering，保留实参个数，避免 unboxed 表示把显式结束位置 0 与省略参数 nil 混淆。没有类型证据时明确拒绝，不静默按列表解释。字符串 `rest` 方法复用同一实现。这不代表所有 Dynamic/host 字符串路径已经支持，也不新增 Unicode 专用 API。

## 验证方式

语义用例放在 core definition `:tests`，包含公共函数、方法和 primitive，覆盖 ASCII、中文、emoji、空串、越界、反向区间、显式结束位置 0、超大整数与参数求值顺序。现有 `check-string-unicode.mjs` 读取同一组 AST，分别运行 native、生成的 JS 与生成的 WASM；WASM 尚无可捕获的 `try` 边界，因此由 host 断言非法表达式 trap，不改写正常调用为 native call。

NaN/无穷大另通过 WASM f64 host 参数验证，属于 ABI 边界测试。继续执行既有 WASM 回归、Rust 测试、全量 `check-all`、格式与 clippy 检查；具体执行结果以 PR 为准。
