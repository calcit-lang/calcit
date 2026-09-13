# WASM 闭包特化 review 修复

PR review 指出两个此前未覆盖的作用域与调用边界：特化参数经 `let` 取别名时会被当作运行时值发射，同名普通值覆盖闭包参数后仍残留旧的闭包绑定；带 inline closure 的 spread call 也会绕过统一特化诊断。

本次调整让两种 `let` lowering 都从当前词法环境解析闭包别名，并在普通值完成求值后清除同名闭包绑定。spread call 在任何 lowering 前检测 inline closure，统一以 `E_WASM_CLOSURE_SPECIALIZATION` 失败关闭。Calcit definition 测试补充闭包别名和普通值 shadowing 语义，Node artifact 测试只补充 spread 边界的 trap 断言。
