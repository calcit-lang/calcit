# WASM nil 判断改用类型证据

## 问题

WASM scalar ABI 让 nil、false 与数值 0 共用相同的零值表示。旧 `nil?` 直接比较运行时数值，因此 `some? false` 与 `some? 0` 被误判为 false；继续增加动态检测无法从相同比特中恢复 Calcit 语义。

## 处理

WASM lowering 现在保存函数参数、局部绑定、闭包与 IIFE 的静态类型证据。具体类型的 `nil?` 在保留一次求值及副作用顺序后生成确定结果；nil-sensitive 泛型 helper 及其转发链在直接调用点按实参类型单态化。跨 namespace 的 Calcit 函数引用沿用现有模块解析，不引入 native call 或额外宿主入口。

开放 Dynamic、未绑定泛型，以及未具体化的一等函数或公开 WASM 导出无法安全区分零值，统一以 `E_WASM_NIL_TYPE_EVIDENCE` 失败。旧 `Optional<T>` 仅在 payload 是确定非零的引用/句柄表示时使用运行时零值判断；`Optional<Number>` 与 `Optional<Bool>` 保持拒绝。非目标泛型 helper 可以保留 trapping slot，供已证明的调用点内联；显式导出和实际入口仍严格拒绝。

## 验证

`some?` 的定义级 `:tests` 覆盖 nil、false、0、Option 与类型化/泛型转发调用。独立脚本从这些测试构造临时 Snapshot，并在 native、生成 JavaScript、core WASM、WASI 0.3 Component/Wasmtime 执行同一组断言；另有显式泛型导出的负例验证错误码。CI 同时运行常规共享契约和真实 Wasmtime 路径。
