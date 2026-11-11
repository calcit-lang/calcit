# 裸 tag 改写为零载荷 enum 分支 (#2701)

- 决定：期望类型是确定 enum 时，裸 tag `:all` 在预处理期改写为 `%:: Filter :all`（`NativeNamedEnumNew`）。来源是 0.42.0 规划中维护者确认的 C6 / F5：只在期望类型明确处改写，歧义报错，范围与 Option 字段自动包装（#2652）一致。
- 位置：具名函数参数（`preprocess` 的逐参数循环，在 nil 检查之前，因此后续参数类型检查看到的已经是构造）、局部函数参数（Local 调用分支）、`%{}` / 直接构造的字段（`coerce_struct_literal_tags`，在 Option 包装之前）、map / `?{}` 改写成 struct 时的字段、函数尾部表达式和尾部 `if` 的分支。
- 判定集中在 `type_rewriting::classify_tag_for_expected`：类型变量、`Dynamic`、`Tag`、core `Option` / `Result` 不改写；`defunion` 有 `Tag` 成员时不改写，否则恰好一个 enum 成员有该零载荷分支才改写，多个报 `E_ENUM_TAG_AMBIGUOUS`。
- 新诊断：`E_ENUM_TAG_UNKNOWN`、`E_ENUM_TAG_NEEDS_PAYLOAD`、`E_ENUM_TAG_AMBIGUOUS`。原来这几种情况只报通用的 `W_FN_ARG_TYPE_MISMATCH`（`got :tag`），看不出是拼写错误还是缺载荷，且 0.42.0 方向是阻断性诊断用 `E_`。
- 分支列表排序输出：struct 字段类型解析出的 enum 定义不保留声明顺序。
- map 改写 struct 的路径没有调用栈，只在能改写时改写，其他情况交给构造字段检查报通用不匹配。
- 验证：`cargo test --test enum_tag_coercion_cli`，fixture `tests/fixtures/enum-tag-coercion.cirru`。
