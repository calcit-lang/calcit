# 局部绑定改名规则 rename-local-v1（#1616）

问题：`tree search-replace` 按文本匹配叶子，区分不了绑定处、使用处与同名遮蔽；Respo `comp-task` 中 `--pick 0` 命中了 `let` 绑定名而不是使用处。定义级改名已有 `rename-definition-v1`，局部绑定没有对应工具。

决策：新增 `calcit fix --rule rename-local-v1 --ns --def --at --to`，沿用 fix 的 preview/apply、`--expect-revision` 与暂存校验。

- 作用域按 core 绑定形式在源码上计算：`defn`/`fn`/`defmacro` 参数作用于函数体；`let` 顺序绑定，后续绑定值与 body 可见；`loop` 初始值属于外层；`&let` 作用于 body。内层同名绑定遮蔽的区域不改写。
- 预处理证明：每个改写的使用处必须在预处理后的定义中是 `Local` 引用（按 source 坐标核对），避免把同名全局定义当成局部。预处理的 `Local` 只带名字索引、不区分同名绑定，所以绑定归属仍由源码作用域决定。
- 拒绝条件：新名字已在作用域中出现（会捕获或被捕获）；使用处在 quote/quasiquote 中；使用处在项目/依赖 macro 或会引入绑定的 core macro 参数中。`when`、`->`、`cond` 等不引入绑定的 core macro 列在白名单中照常改写。
- 只改当前定义的 code；tests/examples 不在范围内。

兼容边界：不新增 `tree` 子命令，不做跨定义改名。白名单之外的新 core macro 默认按“无法证明”拒绝，宁可少改不误改。

验证：`tests/fix_cli.rs::rename_local_rewrites_one_binding_and_its_uses_only` 覆盖 `let` 绑定、`fn` 参数、内层遮蔽、`when` 透传、名字冲突、quote 与 `->%` 拒绝、apply 后严格检查通过以及第二次预览为空。
