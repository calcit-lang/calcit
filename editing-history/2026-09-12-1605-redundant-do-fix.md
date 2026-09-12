# 多表达式 body 中冗余 `do` 的自动整理

## 结论

`defn`、`fn` 与 `let` 的 body 本身按顺序处理多个表达式，并以最后一项作为返回值。`do` 展开为一个空 binding 的 `&let`，所以在这些 variadic body 中额外包装 `do` 不会补充基础类型推断。

`do` 在 `if` 分支、调用参数、binding value 等只接受单个表达式的位置仍有必要；`defmacro` 还可能显式打包 macro body，quote/quasiquote 中的节点则属于代码数据，这些位置都不能按普通运行时 body 改写。

## 实现

- `calcit fix` 新增 `redundant-do-v1`，只匹配已知 variadic body 里的直接 `do` 子节点。
- 每条建议以结构化 splice replacement 表达多个 sibling，并携带 definition、path、subtree fingerprint 与稳定 rule ID。
- 应用时先用 `--expect 'quote do'` 删除 wrapper head，再 unwrap 剩余 body；多个位置按深层、右侧优先顺序执行，避免前一项改写使后续 path 漂移。
- quote/quasiquote 整棵子树不会进入扫描；单表达式 context 不产生建议。
- 继续复用 revision guard、staged transaction 与 scope preprocess；不增加 compiler warning/error 或运行时规则。

## 验证

- Calcit definition `:tests` 覆盖无 `do` 的多表达式 `fn` 与 `let`，验证执行顺序及最后结果的 Number 类型。
- Rust 只覆盖 fixer 的 AST 安全边界、operation 生成、已有 leaf migration 与冲突不变量。
- 独立 strict Snapshot 验证 preview、带 revision 的 apply 和第二次运行空 suggestions，证明规则幂等。
