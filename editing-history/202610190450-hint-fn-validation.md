# hint-fn 的元数据参数验证

Editor 的错误 callback wrapper 和本轮文档检查显示，hint-fn 会静默忽略错误的 Fn wrapper，甚至可给 Number 局部值附加 Fn 合同。额外参数和缺少参数同样被忽略，导致检查通过不代表注解生效。

在原 preprocess_hint_fn 校验现有一参数/两参数形式，复用原 schema 解析器，但只从合法位置提取 schema。两参数目标必须有独立的局部 Fn 证据，不能用希望附加的合同制造可调用性。正式 alpha.5 也确认已知 Fn 返回 String 却可附加 Number 返回合同、完整 strict 通过后运行失败，因此还复用现有签名关系拒绝已知矛盾。此关系检查不等于开放合同的完整有方向证明，后者仍需随门禁统一继续修复。保留 Map 元数据、泛型、async 与 Nil 运行结果，不改为返回函数的新 API。

失败语法通过既有 CLI strict-default 回归，合法行为复用 hint-fn 原附带测试，并增加泛型局部函数保留 Option None / Some nil 区别的附带测试。测试不放宽成任意失败，必须定位 hint 自身的参数错误；不新增检查脚本、fix planner、Dynamic 预算或 unsafe。

自查还以正式 alpha.5 和候选对照复现了裸 Fn 分支拒绝合法 TypeRef 函数别名的回退。改为复用已有非 nullable 函数解析，保留别名/slot 的独立合同，不把 Nullable 函数当作已证明可调用。附带测试验证真实 Fn 别名，现有 CLI 负例继续拒绝 nullable 目标；这不是为某个别名新增特殊规则。

此为可能拒绝旧源码的非 patch 改动，安排在 0.31.0，不回移到 0.30.1。生态迁移依靠源码查询、dry-run 和 revision 守卫，原业务测试仍须通过。
