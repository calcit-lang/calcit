# WASM 静态闭包特化

时间：2026-09-13 07:06 +0800

## 背景

#1029 要求让更多已类型化的高阶调用进入 WASM，同时避免为 `Option.map`、`Result.map` 等 API
逐项增加 backend 规则。现有 inline closure 只能由少数 collection interceptor 消费，普通静态函数调用
无法保留词法捕获。

## 调整

- 从预处理后的函数 schema 识别 callback 参数，并按静态 callee 对普通调用进行源码级特化。
- inline closure 在创建位置保留词法局部绑定；普通值参数仍按调用顺序求值一次，再绑定到被特化函数体。
- 不引入闭包 heap ABI，也不增加 Option/Result 专用 interceptor。
- 动态 callee、可变参数函数、闭包逃逸与递归特化使用稳定的
  `E_WASM_CLOSURE_SPECIALIZATION` 失败关闭。
- 在 `calcit/test-wasm.cirru` 的 definition `:tests` 中增加同一段 Calcit 语义测试，native 与 WASM
  均得到 `16`；底层 trap 断言保留在 Node 脚本，因为它验证的是 WASM artifact 边界而不是语言语义。

## 验证

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- `yarn check-all`
- `target/debug/calcit docs check-md --entry calcit/test.cirru --failures-only docs/run/cli-options.md`
- 同一新增 definition 经 native、生成 JavaScript 与实际 WASM 执行均返回 `16`；四类不支持边界均在
  WebAssembly 实例中稳定 trap。
