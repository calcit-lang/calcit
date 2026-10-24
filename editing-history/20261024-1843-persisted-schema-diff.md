# 结构化 diff 比较持久化 schema，而不是诊断缩写

## 决策依据

`analyze program-diff` 原来用类型的 Display 判断 schema 是否变化。不同值类别可能同为
`custom`，函数 feature 不进入该显示文本，深层类型还会触发诊断截断；缺失标注与显式
Dynamic 的文本也相同。这些真实合同变化会被误报为 canonical-format-only，丢失 Agent
需要的 semantic review 证据。

改为复用 Snapshot 已有的 schema EDN 转换和缺失标注判定。schema 与 FFI 通过同一个内部
结构比较 helper 形成 DiffNode，文本仅用于展示；等价 Map/Set 的迭代顺序不产生变化。
新增/删除定义的 schema 展示也采用持久化形式，不再只显示 custom。
变化项的旧值与新值保留完整单行 EDN，避免深层差异再次被展示截断隐藏；未变化项仍使用
原有简短预览，不扩展结构化输出字段。

方向分析继续使用原来的类型关系。互不包含的类别、feature 或标注意图变化可以要求审阅，
但不据名字或“发生变化”就推断 widening；原 fail-on-widening 门槛保持。
没有新增输出字段、命令、checker、诊断编号或类型检查例外，也不写回 Snapshot。

## 范围与验证

- 现有 program-diff 单元测试覆盖显示碰撞、缺失/显式 Dynamic、超出显示预算的类型，
  以及持久化 Map/Set 顺序与值类别展示；保留原 Fn、macro、FFI 和 widening 用例。
- 现有 CLI format 测试通过受保护事务建立临时 Git fixture，核对默认 human、显式
  Cirru EDN/JSON、两 Git revision 比较和实际 widening 的非零退出；各阶段带字节守卫。
- 这些是 Git/source-metadata/进程协议测试，放在原 Rust tool harness；不为测试 CLI
  而给 Calcit 新增进程或文件系统入口，不用 Rust 重复覆盖语言本身的用户语义。
- 持久化 schema 的表示仍以 Snapshot 既有序列化为准，不把它当成运行时名义身份检查。
  本修复独立于 Struct 来源守卫，不混入它的实现或取消它的审查门槛。

当前使用规则见 `docs/run/cli-options.md` 的 Snapshot 语义差异；发布与实际 review/CI
状态只记录在对应 issue/PR，本记录不代表任何版本或总单已完成。
