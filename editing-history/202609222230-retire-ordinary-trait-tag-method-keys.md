# 普通 trait 方法键退役

- gen-code #27 与 editor #80 已先迁移实际使用的普通 `deftrait` tag 方法键，两仓库主分支 Actions 均通过；其 `defimpl` 已使用 dot 方法键。Calcit 核心的普通 `deftrait` / `defimpl` 现在以 `E_LEGACY_TRAIT_METHOD_KEY` 拒绝旧 tag，给出一对一的 `.method` 迁移提示。
- external-object trait 的 `:field` 是当前宿主属性声明，不属于旧方法键；保留基于 CodeEntry `:ffi` 或已解析 field member 的判别与原有宏展开。`defimpl` 宏中接受 tag 方法键的死分支及旧提示文案已移除。其他历史参数/扁平 method bag 兼容仍需在 #1240 中单独审查，不能把本次收紧视为该 issue 全部完成。
- Rust 测试覆盖普通 tag 拒绝与 dot 保留；Calcit 的 trait 正常路径由现有 `calcit/test-traits.cirru` 验证。`docs/features/traits.md` 中旧 tag method pair 示例也改成 dot，并通过 `docs check-md`。负向编译诊断无法通过正常执行的 definition `:tests` 表达，留在预处理器测试。全量 Rust 测试需要允许本地 HTTP fixture bind；sandbox 内的两项权限失败在授权环境复跑通过。
