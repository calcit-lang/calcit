# 202602272212 — 新增 `tree raise` 和 `tree wrap` 命令

## 改动概要

补齐两个 Lisp 社区（Paredit）常用的结构化编辑操作，完善 `cr tree` 命令集。

## 修改文件

- `src/cli_args.rs`：新增 `TreeRaiseCommand`、`TreeWrapCommand` 结构体；在 `TreeSubcommand` 枚举中新增 `Raise`、`Wrap` 变体。
- `src/bin/cli_handlers/tree.rs`：import 增加两个命令结构体；dispatch 增加两个分支；新增 `handle_raise` 和 `handle_wrap` 函数。
- `docs/CalcitAgent.md`：更新主要操作列表、结构化变更示例、实战重构场景。

## 命令语义

### `cr tree raise <ns/def> -p <child-path>`

等价 Paredit `raise-sexp`。将指定子节点**整体替换掉其父节点**。

- `path` 必须至少一个元素（才有父节点）
- `parent_path = path[..n-1]`，用 `apply_operation_at_path(..., "replace", child)` 实现
- 典型用途：去掉 `if` 只保留某分支、去掉 `let` 只保留最终返回值表达式

### `cr tree wrap <ns/def> -p <path> -e '<template>'`

等价 `cr tree rewrite ... -w 'self=.'`，但更简洁。模板中 `self` 自动绑定为原节点。

- 适合"加一层调用"的常见模式：`wrap -e 'println self'`、`wrap -e 'let ((x self)) x'`
- 当需要引用原节点的**子节点**（不只是整体）时，仍需用 `rewrite --with`

## 知识点

- 三个互相对应的操作：`wrap`（包裹）→ `unwrap`（所有子节点展开）→ `raise`（单子节点替换父）
- `wrap` 与 `rewrite` 的关系：`wrap` = `rewrite` 固定了 `self=.` 的语法糖，降低常用操作的命令复杂度
