# core-macro-alias-v1：退场 core 便利宏

## 依据

#1626 要求清理缩写的日志宏与少用的 `flipped`、`noted`。下游统计（#1626 评论）显示它们并非无人使用：`w-log` 32 个仓库、`wo-log` 16、`w-js-log` 11、`wo-js-log` 9、`flipped` 40、`noted` 12。没有 fix 规则就无法按 AGENTS.md 的节奏退场，现有 `core-function-alias-v1` 只改写函数调用。

## 决定

- 新增 `dbg`（原 `w-log` 的实现），`w-log` 改为转发到 `dbg`。
- `core-macro-alias-v1` 把调用写回宏自身的展开：`w-log x` → `dbg x`，`wo-log x` / `wo-js-log x` → `x`，`flipped f a b` → `f b a`。`w-js-log` 交给 `js/console.log` 的是宿主对象，与 `dbg` 的打印不同，只给 review。
- `w-log`、`wo-log`、`w-js-log`、`wo-js-log`、`flipped` 标 `:deprecated`。
- `noted` 保留：`calcit query anchors` 读取 `noted @anchor:<name>`，它是现有功能的一部分。
- 上下文判定复用 `case-default-to-match-v1` 的 `unstable_macro_context`，传入本规则视为原样转发的宏；`let-sugar`、`let-destruct` 只把值原样交给 `let` / `let[]`，加入共享的参数保持列表。

## 验证

- `tests/fix_cli.rs` 新增集成用例：五种改写、嵌套合并、quasiquote/quote/线程宏/未知宏的 review、应用前后附带测试结果一致、重复预览只剩 review 项。
- 下游副本试跑（临时把 deps.cirru 版本改为当前 CLI）：respo-router `listen!` 中的 `flipped js/setTimeout 0 ...` 自动改写；triadica-space `transform-3d` 中两处 `wo-log` 在 `let-sugar` 内自动改写。其余定义因下游自身的严格检查错误无法预处理，与本规则无关。
