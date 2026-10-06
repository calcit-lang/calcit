# 已知空容器的同类证明

关联 #1795、#1529、#1553 与 TopixIM/diary#69。

## 决策

Diary 将节假日 Set 的 spread 改为普通 typed fold / `.union` 后，已发布 alpha.14 拒绝初始 `assert-type (#{}) (:: 'Set 'String)`。独立 proof 的 source-shaped 逐成员校验明确排除空构造器，与既有参数和 fold 上下文的空容器规则不一致。

删除这项排除：已知同类构造器的所有成员满足合同，零成员自然满足。没有改变推导出的开放容器类型，没有借返回声明验证 producer，没有新增 public rule、诊断码、类型或 registry。caller 泛型 rigid policy、mutable Ref 和无 payload Enum 不在范围内。

## 验证与兼容边界

定义附带的 Calcit 测试覆盖空 List、Set、Map、词法别名/分支及真实消费者形状的 `.union` fold。现有 known-assertion runner 回放相同 AST 到 native / 生成 JS，并校验既有 assertion/return/callable proof。未知容器、非空开放成员、错误种类、错误成员和 shadowing 继续拒绝。

旧测试把实际空 Map producer 当成“只有返回声明的未知 producer”；改为带 Dynamic 成员的非空 Map，保留声明不能借给调用者、缓存预热和矛盾返回的负例。不是删除原安全门禁，也不宣称发布后消费者完整 strict workflow 已通过。候选和正式版本的验证分别记录在 PR。
