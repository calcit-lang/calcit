# 嵌套写入迁移到已有方法

## 范围与依据

沿 #2187 已确认的方法优先路线，迁移 assoc-in / update-in 的内部 assoc 调用。两者原本就是开放 Dynamic 输入/输出；保持 schema、examples、原 :tests 和 API 不变，不把这一步当作公共旧入口退场。

Map/List 使用已有 .assoc；开放 Enum 沿用旧 assoc 实际使用的 &enum:assoc，因为仅有 kind 信息不足以提供名义方法 origin。Struct 的原先明确拒绝保持不变。不增加 originless 例外、unsafe 或新的 Dynamic 合同。

## 求值与错误语义

先求原嵌套值/回调，再检查写入容器和索引。旧 assoc 的参数本来是 eager 求值：例如 update-in 用开放 List 和非法键调用时，回调必须先执行一次，再报告 Number index 错误。String/Enum 等边界也保留原错误文本；assoc-in 对 List 非数字键的 contains? 错误仍早于写入。

使用 tree rewrite 保留原子树，事务经过 dry-run、scoped revision 和 post-query。新增 fixture :tests 验证 List/Enum 非法键的回调顺序、String/Set 错误及 Enum 成功写入，并沿用已有 method-contract 回放 tag，不新增 checker。

## 验证与边界

同一个新增测试表达式在迁移前候选及完整类型原型上均通过。原 13 项测试及新增回归使用现有跨后端运行器验证；原型已通过 native/JS，WASM/WASI 保留原支持范围，无新增排除。独立 main 候选仍须单独验证，不用原型结果替代。

完整类型原型仍有 assoc、dissoc、dissoc-in 的失败；本次源码迁移不宣称这些合同或消费者迁移已完成。独立 PR 继续保留最新 HEAD CI/review 与精确 main 的验收门槛。
