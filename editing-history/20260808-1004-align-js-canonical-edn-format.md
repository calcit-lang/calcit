# Align JS Canonical EDN Format

`cirru_edn 0.8` 在 Rust 端将 enum discriminator、typed enum name/variant 和
nominal struct name 规范化输出为 symbol quote。JS runtime 原先保留 `:` tag 写法，
导致同一套 Cirru EDN fixture 在 native 与 JS 后端产生不同结果。

调整 `ts-src/js-cirru.mts`：仅在 struct/enum EDN shape header 中通过
`CalcitSymbol` 输出这些结构标识符；普通 payload tags 仍保持原有 tag 格式。

验证：`yarn tsc`、`yarn try-js`、`cargo test`。