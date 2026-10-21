# 限定 Snapshot 序列化后的 schema 校验边界

## 问题

#2176 的合法业务函数在 `eval` 中可以通过，但通过官方编辑事务保存时，
序列化后校验器把 quoted code 中的 `(:schema (str text))` 当成类型声明，
随后以 `str` 不是 EDN 类型运算符为由拒绝。示例和附着测试中的同名字段也会受到影响。
更换业务字段名、改写为 `assoc` 或关闭校验都不应成为迁移要求。

## 实现边界

保留已有类型校验函数、primitive schema 兼容处理及多余 quote 的拒绝。
只将原来无范围的递归扫描改为沿 canonical Snapshot 的
`:files → FileEntry.:defs → CodeEntry.:schema` 定位真实元数据；
保留真实字段的树路径用于错误定位。

该函数仍用于序列化器生成的 Snapshot 内容，不是通用 EDN 校验入口，
也不代替加载器或 schema 编辑器。Map 与 record 两种元数据存储形式均覆盖。
普通 `:code`、`:examples`、`:tests` 和 FFI 数据不因碰巧包含同名字段而成为额外类型声明。
Namespace 的 `NsEntry` 不拥有类型 schema。

## 验证

- 新增 CLI 回归先在原实现上复现事务 dry-run 失败，再覆盖 revision 守卫应用、格式化及定义数据不变。
- 附带 Calcit 测试和示例以普通表达式验证 Map 构造、声明 Struct 字段访问及实际结果，并运行完整定义检查。
- 真正非法 schema 的直接写入、事务预览和事务应用仍失败；同一事务中先暂存的 doc 不落盘。
- 扩展原序列化测试，确认 Map/record 形式的真实元数据仍拒绝 double-quoted 泛型。
- 完整 edit CLI 41 项、Snapshot 单元测试 84 项及 all-target Clippy/格式门禁通过；不新增检查脚本。
- 对正式 Respo alpha 的 Snapshot 临时副本执行 guarded dry-run/apply、格式化与定义查询；
  新增业务字段保持原样，原 RespoEvent 元数据与权威 Snapshot 不变。
  这是本地未发布候选的写盘回归，不冒充 Respo 全业务或正式工具链配对验收。

## 兼容性

只解除对合法业务语法的误判；不修改语言类型关系、Snapshot 数据结构、
公共入口、迁移规则、Dynamic 范围或真实元数据的校验要求。
