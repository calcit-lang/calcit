# Tag/Symbol 转换的受控迁移

## 决策

`to-tag` 与 `to-symbol` 只承诺 `String` 输入，而旧 `turn-tag` / `turn-symbol` 的运行时兼容输入更宽。因此不能靠文本替换或旧入口的返回类型推断来迁移。显式规则 `core-identity-conversion-v1` 仅检查 definition `:code` 中完整的内建 Proc 调用，证明单一参数为 String、源码上下文稳定后，才把调用头改为限定名 `calcit.core/to-tag` / `calcit.core/to-symbol`。它不加入已发布 preset，也不增加新的顶层 CLI 命令。

## 保留边界

Dynamic/未知类型与未知 macro 给人工审阅；已知非 String、quoted 数据、definition macro、一等函数值以及同名局部绑定不自动改写。限定名避免 import 或局部命名捕获。旧调用与新调用在已证明 String 的路径上都执行同一底层转换且参数仅求值一次；WASM 当前缺少动态 Tag/Symbol intern，因此此规则不宣称赋予 WASM 支持。

## 验证

临时 Snapshot 使用结构化 CLI 构造 String 正例、Dynamic 与 Tag 负例、未知 macro 和 quote；两个正例各有定义内 Calcit `:tests`，改写前后均执行。验证 JSON 预览不写入、revision 错误拒绝、只应用可证明的两项、应用后 native 运行与再次预览幂等。完整 Rust/JS/Agent 回归及文档检查在 PR 前执行。
