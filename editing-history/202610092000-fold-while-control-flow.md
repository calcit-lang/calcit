# fold-while 与 ControlFlow 替代 foldl-shortcut

## 依据

#1626 指出 `foldl-shortcut list init default f` 的两处不合理：reducer 用 `:: Bool acc` 元组表示停止或继续，读者要记住 `true` 的含义；从未停止时返回额外的 `default`，而不是累积值，和其他 fold 不一致。下游默认分支中只有 Recollect 使用它。

## 实现

新增名义 enum `ControlFlow`（`:continue 'A`、`:break 'A`），以及 `fold-while xs init f`：reducer 返回 `ControlFlow`，`:break` 立即返回其值，从未 break 时返回累积值，空 List 返回初始值。实现用定义内部的 `loop`/`recur`，因为 WASM 不支持对定义自身的递归特化，而内部 `loop` 可以在 native、JS、WASM 上运行。

`foldl-shortcut` 标为 `:deprecated` 并保留：两者语义不同（`default` 与累积值），没有能保持语义的自动改写，迁移按 `docs/run/upgrade.md` 的对照人工完成。core 内 `any?`、`every?` 仍使用原生 `foldl-shortcut`，它们的 `default` 与初始值相同，留到删除旧入口时一并替换。

## 验证

- `calcit.core/fold-while` 的 `:tests` 覆盖 break、耗尽、空 List、break 后不再调用 reducer、累积值类型；native、JS 通过，WASM 中依赖 `atom` 的一条按 unsupported 排除。
- 循环中以 `str` 拼接数字的 String 累积值在 WASM 上 trap，测试改用 `&str:concat`，该 parity 问题另行记录。
