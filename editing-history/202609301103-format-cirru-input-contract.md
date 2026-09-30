# `format-cirru` 的外层 List 契约

## 缺口与决策

公开 core schema 已写明 `format-cirru: List -> String`，native 运行时也要求转换后的 Cirru 顶层为 List；但底层 `CalcitProc::FormatCirru` 参数签名仍是 Dynamic。结果 `format-cirru 1` 通过严格预处理，直到运行时才报类型错误。

将底层签名收紧为 List，与公开 schema 和顶层行语义一致。递归语法节点包含 String/List，现有 List 元素开放边界保留；`format-cirru-edn` 的数据序列化输入仍为 Dynamic，不连带收紧。不新增 formatter、检查规则或自动改写，也不把 `CirruQuote` 误称为此入口已证明的参数类型。

## 验证

Calcit definition `:tests` 验证嵌套 Cirru 行格式化和 String 返回；Rust 负例验证 Number 在运行前产生 `W_PROC_ARG_TYPE_MISMATCH`。完整 native/JS/WASM 与文档门禁确保没有改变合法 List 的格式化语义。
