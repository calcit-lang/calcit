# Wasm 静态 Option/Result 方法语义

## 发现

Calcit typed core 已经会把已知 `Option` / `Result` receiver 上的 `.map` 和 `.unwrap-or` 解析为具体的 `option:*` / `result:*` 定义。Wasm 后端现有 Enum 表示与具名函数引用也能执行这条路径，不需要 runtime trait registry 或新的 backend 类型规则；此前缺少的是跨 native、JS、Wasm 的用户语义契约。

无捕获的具名回调可以经现有函数表传入 `option:map` / `result:map`。局部闭包作为普通函数参数仍需要后续统一的静态函数特化，不能为了本切片增加 Option/Result 专属闭包规则。

## 变更

- 新增一个带完整 schema 的数值回调。
- 新增 `test-static-option-result-methods`，通过用户侧 `.map` / `.unwrap-or` 写法同时验证 `Option` 与 `Result`。
- 把主要断言放在 definition `:tests`，并把同一 export 接入 Wasm 端到端检查。

## 验证重点

- typed preprocessing 选择具体 Option/Result method target，而不是 runtime trait dispatch。
- `Option.some` 与 `Result.ok` 复用现有 Enum layout，并在 Wasm 中得到与 Calcit 相同的数值结果。
