# 开放路径的回调输入证明

Tabletwo 的完整 strict 揭示 #2046：`update-in` 从 Dynamic 容器读取 `Option<Dynamic>`，
却将回调输入/输出都声明为 T。明确接收开放值并在使用前解码的回调仍被模板证明拒绝。
维护者已批准在 0.30.0 修订真实输入/输出关系并通过现有事务提供受保护迁移。

公开 fallback 使用 `Fn<Option<Dynamic>, R>`，不把输出 R 当作输入证据。
已证明静态 Map 路径在既有 checked-call contract 中绑定叶子类型，并复用原 Map
路径重建证明；字面量空路径绑定整个接收者，回调可输出其他类型。不新增检查编号、
API、TypedPath AST、固定迁移规则或豁免。未知路径与混合遍历仍须在实际使用前校验。

验证优先放在 update-in 的 Calcit :tests：开放输入先解码、输出不同型、精确 Map 叶子、
空路径、单次求值顺序和原缺失/集合语义。原 check-known-assertion harness 增加窄回调、未校验输入、
错误静态叶子和重建输出的拒绝条件，防止修正 schema 后弱化检查。
原 API 历史合同、完整跨后端回归和 Tabletwo 真实验收仍须执行，不以聚焦通过代替。

普通 strict 负例还揭示：未写 hint 的匿名回调没有继承声明中显式开放的输入标记，
导致 Option<Dynamic> 被当作 Unknown，数值消费只能由显式 proof 发现。现在将已声明
Fn 参数中的显式 Dynamic 传给既有 callback context 标记；不把 generic 替换出的洞
或推断 callable placeholder 当作 Dynamic 声明，不继承 callee 的 feature 权限。
Snapshot schema 与源代码 body hint 两种声明都须提供该证据；已用独立最小例确认
只读取 metadata 会漏掉 body hint。错误 callback 已有参数警告时，路径 lowering
不再继续，避免将真实源错误变成 CALCIT_LINT_CORE 的内部 invariant 错误。

静态 Map、空路径及原嵌套叶子已实际通过 WASM 与 Wasmtime 49.0.2 的 WASI 0.3 回放，
移除原嵌套叶子对应的过期排除。开放遍历的 E_WASM_NIL_TYPE_EVIDENCE 及 Ref trace 的
unsupported 均保留有实测依据的逐例排除，不缩小输入或将开放路径描述为已支持。

完整 core 回归发现四个旧内部 pair API 测试从声明为 Dynamic 的 callback 输入直接
读取数值。测试保留原 API 调用和结果断言，在数值运算前使用既有 number? 守卫。
这修正测试自身的开放边界，不放宽 callback 证明；用同一守卫保留原 WASM filter
覆盖，避免引入 WASM 尚不支持的 decode-map-as，也不新增排除项。
review 另要求明确文档范围：输出独立适用于开放路径和空字面量路径；已证明的
非空同质 Map 路径要求 callback 返回原叶子类型。
