# `parse-cirru-list` 的输入类型证明

## 缺口与决策

公开 Snapshot schema 已写明 `String -> List<Dynamic>`，运行时也只接受 String，但底层 `CalcitProc::ParseCirruList` 签名仍是 `Dynamic -> List`。因此严格预处理会放过 `parse-cirru-list 1`，运行时才报类型错误；同族 `parse-cirru`、`parse-cirru-edn`、`json-parse` 都会在预处理阶段给出参数类型告警。

本批只将该 proc 的参数签名收紧为 String，不增加解析入口、检查器或别名，也不改变返回结构。返回值确实是递归的 List/String Cirru 数据，现有 `List<Dynamic>` 边界仍是诚实的开放结构，不误改为 `CirruQuote`；若调用方需要类型化业务数据，应使用相应的 checked parser/decoder。

## 验证

Calcit definition `:tests` 断言合法文本仍得到嵌套列表，并保留既有返回类型；底层 Rust 负例验证 Number 在运行前产生 `W_PROC_ARG_TYPE_MISMATCH`。进一步运行 core tests、完整 Rust/JS/WASM 检查和文档门禁，确认没有改变解析结果或 backend 行为。
