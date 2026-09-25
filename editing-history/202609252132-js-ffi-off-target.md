# 双宿主 Snapshot 中的 JS FFI target 边界

Calcit 的一个 Snapshot 可以同时提供 Node 和 browser entry。模块内 JS 文件只属于其声明的宿主；不能因为另一个 entry 构建时枚举到同一 Snapshot 的定义，就将异宿主实现或 `:modules` import 放入产物，也不能让实际跨宿主调用在静态检查中通过。

本次在调用预处理阶段检查被调用 JS FFI 定义的 `:target`，并在 JS 生成阶段跳过当前 entry 不适用的实现。其余 Fn schema、`:js-ffi`、文件资源和模块 specifier 验证保持原样。回归在同一模块中分别放入 Node/browser 文件定义，检查未调用的异宿主实现不阻断构建、产物不含异宿主 import，实际跨宿主调用仍报 `E_JS_FFI_TARGET_MISMATCH`。

外部验证使用 js-ffi 的 Node `path-join` 文件适配器：Node 与 browser entry 共用同一 Snapshot，完整模块测试通过。该模块变更应在本修复进入已发布的 Calcit 版本后再合并，避免依赖未发布的 core 行为。
