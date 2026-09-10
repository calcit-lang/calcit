# JS native list runtime aliases / JS 原生列表运行时别名

## Context / 背景

`&list:append`, `&list:prepend`, and `&list:butlast` are distinct
`CalcitProc` variants for the native and WASM backends. The JavaScript runtime,
however, intentionally exposes the same implementations under the public
`append`, `prepend`, and `butlast` exports. Generic JS proc-name escaping emitted
references such as `_$n_list_$o_append`, which do not exist in
`@calcit/procs` and only fail later when an ESM bundler resolves imports.

`&list:append`、`&list:prepend` 与 `&list:butlast` 在 native/WASM 后端使用独立的
`CalcitProc` 变体，但 JavaScript runtime 复用公开的 `append`、`prepend` 与
`butlast` 导出。通用名称转义会生成不存在的 `_$n_list_$o_append` 等引用，直到
下游 ESM bundler 解析导入时才暴露错误。

## Decision / 决策

Map the three internal variants to their existing public runtime exports at the
JS emitter's proc-value boundary. This covers both direct calls and higher-order
uses without duplicating runtime implementations or changing native/WASM
semantics. Regression tests assert both emitted forms for every alias.

在 JS emitter 的 proc value 边界将三个内部变体映射到已有公开 runtime 导出，
同时覆盖直接调用与高阶函数值，不复制实现，也不改变 native/WASM 语义。回归测试
逐一断言两种生成形式。
