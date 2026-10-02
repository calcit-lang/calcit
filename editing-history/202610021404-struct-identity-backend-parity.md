# Struct 定义身份与 backend 值语义

## 决策

原生值已使用名称、定义路径、字段名和值作为相等与哈希的共同语义。JS 和 WASM 继续实现这一契约，而不是将打印名称、对象地址或运行时分配顺序当作定义身份。

- JS 在顶层值初始化后为尚无身份的 StructDef 绑定 `namespace/definition`；已有身份的别名原样保留。字段更新和 trait 组合保留该身份。值相等、哈希以及 Struct 比较的身份前缀使用此路径。
- WASM 按名称、可选定义路径和有序字段表生成确定性的布局身份。短名称 tag 仍供 `get-name` 返回；字段查找、构造、受检 EDN 解码和 Component lifting 使用同一布局表。
- 递归相等复用既有值比较器。`&hash` 与 Map key 统一复用递归哈希；Map key 比较也复用值比较器，避免相等的重新构造值无法找回原有 key。列表与名义字段有序，Map entry 和 Set 元素无序；数值零统一哈希，不区分 `-0` 和 `0`。

## 边界

热重载保持路径身份，但仍比较字段布局和值。无顶层绑定的定义没有被授予某个同名顶层定义的身份。哈希数值是 backend 内部细节，不承诺跨 backend 数值一致，也不是持久化协议。普通 `=` 保持同静态类型约束；跨名义类型的运行时负例仅以底层 `&=` 验证内核契约，不放宽用户类型规则。

没有扩大 WASM 的通用排序支持；JS 既有容器排序仍需单独审查，不能把本次身份前缀修复表述为所有复合类型的排序对齐。typed EDN 的模块别名名称解析另见 #1663，本次用完整名义类型路径验证身份恢复，不新增 resolver。

## 验证

共享 `calcit/test-wasm.cirru` definition `:tests` 覆盖跨 namespace 同名值、重复构造、嵌套值哈希、Map key、容器递归哈希、同名不同布局与 typed EDN 身份恢复，在现有 native/JS/WASM 运行器回放。Component fixture 另以 Calcit 定义比较 lifted record 与直接构造值，并由现有宿主 ABI 测试传入实际 record。

JS runtime 元数据测试验证重建定义、别名、字段更新和 trait 组合；这些对象身份条件属于 host 边界。完整 Rust、Clippy、JS 集成与 Agent CLI 按仓库流程运行，最终状态与命令保留在 PR，不以本记录替代验证结果。

共享 target 的 JS FFI 搬迁 fixture 显式连接当前工作树的 node_modules，避免 Node 按真实路径向上找到其他 checkout 的旧 runtime；不改动其他工作树的依赖或缓存。
