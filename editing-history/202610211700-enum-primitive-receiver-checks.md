# Enum 原语的专用检查不再替代通用参数合同

#2181 报告：参数声明为 `List<String>` 时传给 `&enum:nth`，严格检查通过，native 与 JS 运行时才抛错。`&enum:count`、`&enum:assoc` 等同类原语会报告 `W_PROC_ARG_TYPE_MISMATCH`，差别只在 `check_proc_arg_types` 的分支：`&enum:nth` 调用专用的 `check_enum_nth_bounds` 后提前返回，`%::` 调用 `check_enum_construction` 后同样返回，两者都跳过了按 proc 签名逐个检查参数的通用循环。`check_enum_nth_bounds` 本身是空实现（动态 Enum 不携带各变体载荷长度，无法静态检查越界）。

修复方向是让专用检查只追加证据，不替代通用合同：删除空的越界检查函数及其只验证“无警告”的单元测试，`%::` 在变体/载荷检查之后继续进入通用参数检查。这样 `&enum:nth` 的 `:enum` 与 `%::` 的 `:enum-def` 由同一条签名规则覆盖，没有新增诊断编号或特殊分支。结构体原语（`&struct:get` / `&struct:nth` 等）的提前返回保持不变：它们是编译器 lowering 目标，严格源码直接使用已由 `E_RAW_PRIMITIVE_IN_TYPED_CODE` 拒绝。

验证放在 `src/bin/cr_tests/type_fail.rs`：`:tests` 只在检查通过后运行，无法表达预期的预处理诊断。用例覆盖 `&enum:nth` 与 `%::` 的已知 List 接收者被拒绝，以及匿名 Enum、`Option` 和 `defenum` 值的正向对照。仓库内 `calcit/*.cirru`、core tests 与文档代码块的检查结果与修改前一致。
