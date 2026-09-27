# `recur` 参数类型遵守词法函数契约

## 问题与决定

Quamolit 的 `sample-curve-segment` 曾把 `recur` 的第一个 `Vec2` 参数与第五个 `Number` 参数错位。既有检查只验证参数数量；普通函数调用已有的类型关系没有应用到尾递归调用，导致 `analyze check-public` 放过问题，运行时才在数值比较失败（calcit#1423）。

预处理在进入函数体时保存该函数参数的类型序列，检查 `recur` 时复用现有实参兼容性关系。嵌套函数使用自己的参数序列，不把外层函数的签名套到内层；源码诊断指向错误调用。这里只补上缺失的调用边，不引入并行类型系统、动态类型统计或新的公开分析入口。

## 边界

固定参数的具名函数可以按其 `Fn` schema 逐项检查。带 `&`、`?` 等标记参数的函数沿用原有特殊 arity 边界，不在本次改动中推断可变参数与固定参数的对应关系。显式 `Dynamic` 仍是开放值，不能仅凭递归调用反推为具体类型。编译器只诊断错误，不写回 Snapshot 或替用户调整实参顺序。

## 验证

- Calcit definition `:tests` 覆盖正确类型和顺序的递归，并由核心 native/JS 路径运行。
- Rust CLI 集成测试覆盖错序时严格检查和 `analyze check-public` 的结构化诊断、预期/实际类型与源码位置。
- 在隔离的 Quamolit Snapshot 副本上，正确调用为 14/14 PASS；复现的错序调用为 13/14 PASS，准确报告第 1 与第 5 个实参不匹配。原项目工作树未改动。
- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test -q`、`yarn check-all` 与相关 Markdown 示例检查均通过；PR 最新 HEAD 的 Actions 和 review 仍需独立确认。
