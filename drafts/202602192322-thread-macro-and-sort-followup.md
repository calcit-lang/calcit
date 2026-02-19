# 202602192322 thread macro and sort followup

## 本次修改摘要

- 线程宏步骤识别完善：提炼并统一为 `thread-step?`，覆盖 symbol/tag/fn/method 与 list step。
- 修复 `->` / `->>` 在 access method、proc/fn 与 tag step 场景下的可用性与一致性。
- `sort` 恢复单参数自然排序，同时保留双参数 comparator 排序。
- `sort` comparator 异常分支保持可恢复错误（返回 `CalcitErr`），避免 panic。

## 涉及文件

- `src/cirru/calcit-core.cirru`
- `calcit/test-macro.cirru`
- `src/builtins/lists.rs`
- `src/calcit/proc_name.rs`
- `calcit/test-list.cirru`
- `src/calcit/record.rs`

## 验证结果

- `yarn check-all` 通过。
- 定向验证：`sort ([] 3 1 2)` 可执行并返回自然排序结果。

## 经验记录

- 宏步骤校验应抽取成单一判定函数，避免 `->` 与 `->>` 分叉。
- 核心函数文档与实现必须同步（尤其是可选参数/单参数退化路径）。
- 对高频基础能力（如 sort）优先保证“可恢复错误”而非 panic，以降低运行时中断风险。
