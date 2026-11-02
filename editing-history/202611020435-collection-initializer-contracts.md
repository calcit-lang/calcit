# 顶层集合初始化合同

Respo UI 的 0.34.0 迁移将 31 个静态样式常量声明为 `Map Tag StyleValue`，其中 StyleValue 为 String/Number/Tag。官方工具链拒绝同质 Bool Map，却接受混合 Bool/Tag Map；原完整 strict 也漏检，见 #2532。

原因是顶层值先使用集合整体推导检查合同；混合成员合并为 Dynamic 后，只得到开放证据，原迁移策略便接受该关系。保留开放值策略，但明确矛盾不能随这个合并丢失。

将现有集合字面量逐项访问抽成一个私有函数，原返回值/字段证明继续使用同一个访问逻辑。顶层值在整体证据开放时，用既有 TypeProof 关系检查已知成员和嵌套集合，只拒绝明确 Mismatch；不把开放成员提升为证明，不执行初始化器，不增加诊断、proof 状态、CLI 开关、迁移规则或检查脚本。

合法混合 Map/List/Set 和嵌套值放在原 fixture 的 definition tests，通过共享 native/JS runner 回放。拒绝程序无法执行附带测试，留在原 strict CLI 测试，覆盖普通入口、公开 namespace 与 JS 生成前拒绝，以及开放成员仍保留、其他已知矛盾仍拒绝。真实 UI 候选和 width=false 控制使用同一未发布编译器验证，正式发布验收仍单独完成。

本次评审补充覆盖了期望类型自身为 union 的情况：复用 checked_members 的唯一运行时种类选择集合成员，List/Tuple 同属 List 种类，再用同一逐项访问检查其合同。新增 List/Tuple union 的合法附带语义及拒绝回归，保留开放成员策略，不把容器 union 的整体开放推导当作成员均合法。

补充核实短名称成员：将 `Sequence` 中的 `(List app.collection-value/Scalar)` 改为用户自然书写的 `(List Scalar)` 后，原候选错误拒绝合法混合 List。现在复用既有 namespace 类型引用解析，在 union 声明 namespace 解析成员，保留原受检关系。共享附带测试覆盖合法短名称，原拒绝/开放证据回归沿用同一 fixture。

Resolve nested member references in the declaring union namespace using the existing resolver. The short-name fixture reproduces the earlier false rejection; valid attached tests and invalid/open CLI controls share that fixture.

跨命名空间嵌套 union 的后续评审核实：同名 Scalar 故意遮蔽时，旧候选会拒绝合法 List 并接受 Bool/Symbol List。现在在既有 UnionMember 中保留展平前的声明 namespace；集合访问复用它解析嵌套短名，仅无来源时回退外层。没有新增证明状态或检查入口。原 fixture 新增遮蔽 namespace 与一项共享测试，CLI 扩到17组×普通/公开/JS控制；共享 native/JS各2/2、Clippy通过，原20个namespace完整视图/元数据保留。最新冻结源码全量门禁仍单独完成，不沿用源码演进期间的旧运行。

Preserve the owning namespace on each existing flattened UnionMember. A shadowing fixture reproduces both false rejection and false acceptance before the fix. Shared native/JS replay passes two tests per backend, the original CLI suite covers seventeen controls, and all twenty original namespace views and metadata are preserved. Full frozen-source gates remain separate from obsolete runs spanning source changes.
