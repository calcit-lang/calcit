# 顶层集合初始化合同

Respo UI 的 0.34.0 迁移将 31 个静态样式常量声明为 `Map Tag StyleValue`，其中 StyleValue 为 String/Number/Tag。官方工具链拒绝同质 Bool Map，却接受混合 Bool/Tag Map；原完整 strict 也漏检，见 #2532。

原因是顶层值先使用集合整体推导检查合同；混合成员合并为 Dynamic 后，只得到开放证据，原迁移策略便接受该关系。保留开放值策略，但明确矛盾不能随这个合并丢失。

将现有集合字面量逐项访问抽成一个私有函数，原返回值/字段证明继续使用同一个访问逻辑。顶层值在整体证据开放时，用既有 TypeProof 关系检查已知成员和嵌套集合，只拒绝明确 Mismatch；不把开放成员提升为证明，不执行初始化器，不增加诊断、proof 状态、CLI 开关、迁移规则或检查脚本。

合法混合 Map/List/Set 和嵌套值放在原 fixture 的 definition tests，通过共享 native/JS runner 回放。拒绝程序无法执行附带测试，留在原 strict CLI 测试，覆盖普通入口、公开 namespace 与 JS 生成前拒绝，以及开放成员仍保留、其他已知矛盾仍拒绝。真实 UI 候选和 width=false 控制使用同一未发布编译器验证，正式发布验收仍单独完成。
