# 对齐 runtime schema alias 验证

## 问题

静态类型关系会把 `'namespace/definition` 展开为 definition 的非 Dynamic schema，但 Native runtime 的 Struct/Enum payload 验证只把 `TypeRef` 当作名义 Struct/Enum 名称。相同的 List 或 Fn schema 以内联形式可以通过，改成别名后却在 Native 构造阶段失败；JavaScript 后端没有这项偏差。

## 修改

- 在统一的 runtime value/type relation 中保留名义 Struct/Enum 快速路径，并在不匹配时使用现有 schema alias resolver 展开底层类型。
- 复用静态关系已有的 alias guard，使循环别名确定地返回不匹配，不递归溢出，也不退化为 Dynamic。
- 回归测试覆盖 collection、callable、trait 和循环 alias；不为某个 constructor 增加专用例外。
- 中文类型指南说明 alias 没有额外运行时包装，Native 与 JavaScript 使用同一底层 schema 语义。

## 结果

Struct 字段和 Enum payload 等现有 runtime 验证入口自动获得一致行为。迁移项目不需要为 Native 添加 coercion，也没有引入新的分析分类或独立类型模型。
