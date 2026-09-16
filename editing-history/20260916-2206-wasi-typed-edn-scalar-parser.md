# WASI 类型驱动 Cirru EDN 标量解析

## 背景

`try-parse-cirru-edn-as` 在预处理阶段已经生成闭合的 `DataShapeGraph`，WASM 后端不应再建立平行 schema，也不应先解析成 `Dynamic` 再做运行时类型探测。

## 实现

- WASM codegen 直接读取预处理表达式携带的 `DataShapeGraph` root。
- 首批支持 nil、Bool、Number、numeric refinement、`|text` String 和编译产物已知 tag；Unit 与 native 一样返回 `:err`，因为 Cirru EDN 没有 Unit 表示。
- 同时接受 bare scalar token 与 native formatter 的顶层 `do token` 形式。
- 输入最多 64 KiB；超限、语法错误、数值越界和未知 tag 都返回稳定的 `Result :err`，不使用 trap。
- Numeric refinement 在解析后按统一的 f64 runtime 语义检查整数性、范围或 Float32 精确表示能力。
- Result 继续复用 `%ok` / `%err` 的现有 enum heap layout，没有增加 CLI 或语言入口。

## 验证

- Calcit `:tests` 覆盖 native 标量语义。
- 真实 Wasmtime fixture 覆盖成功值、错误状态与超限输入不 trap。
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `scripts/test-wasi-preprocess.sh`

## 后续

下一步沿用同一 parser 增加 quoted/escaped String，然后实现带 token/depth/allocation budget 的 List、Map、Struct 与 Enum 递归节点，为真实 WASI 文件 roundtrip 提供 typed decode。
