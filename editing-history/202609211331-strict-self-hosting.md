# 严格类型默认自举 / Strict-default self-hosting

## 中文

- 将 core、原生与 JS 测试、IR、WASM、WASI 能力检查和 benchmark 的普通入口改为默认严格类型，不再依赖全局兼容开关；仅在明确验证兼容模式的独立回归中保留开关。
- 补齐 core 与第一方 Calcit Snapshot 的类型 schema、明确的 host-object trait 边界和必要的显式转换，保留用户可见行为，并在 `:tests` 与跨 backend 运行中验证。
- 修正类型推导中具名局部回调、泛型绑定、空容器和宏展开类型引用的边界；WASM 发射函数体时忽略 `hint-fn` 元数据。
- 移除依赖过期生成文件及旧 `.method` 形式的检测脚本；收敛余数 benchmark 的输出，避免将已相同的实现标成动态与静态性能对比。
- 为已有 core 动态类型审查位置更新质量基线，不扩大指标规则；普通入口增加最小的兼容开关防回退检查。

## English

- Run ordinary core, native/JS, IR, WASM, WASI capability, and benchmark entrypoints under strict typing by default; retain the switch only in explicit compatibility regressions.
- Add schemas and explicit host-object boundaries to first-party Calcit Snapshots, preserving observable behavior and validating it through attached tests and backend runs.
- Correct inference for named local callbacks, generic binding, empty containers, and macro-expanded type references; ignore `hint-fn` metadata when emitting WASM function bodies.
- Remove a stale generated-JS detector and stop reporting identical remainder implementations as dynamic-versus-static performance comparisons.
- Update the reviewed core quality baseline without expanding analyzer rules, and guard ordinary entrypoints against reintroducing the compatibility switch.
