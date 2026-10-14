# 开放路径写入保留已证明的 Map 前缀

## 依据

Rebase Hell 接续迁移时，多个 `Map<Tag, Dynamic>` updater 的 `assoc-in`
结果被推成 Dynamic。依赖-free 最小 Snapshot 在正式 alpha.3 上运行值断言成功，
完整 strict 仍拒绝其 Map 返回证明；不是通过函数返回声明即可消除的错误。

## 设计

复用 checked collection contract，沿非空字面量 path 核对已知 Map 的 key。
到达 Dynamic payload 后，只证明成功写入保留已经核对的 Map 前缀；不证明
未知节点的类型或字段存在，不展开这一段路径，也不改变公开 walker 的失败和求值。
assoc-in 与 update-in 共享该前缀关系；update-in 的开放 callback 输入仍为
Option<Dynamic>，独立校验其合同，不用 callback 输出证明输入。

空路径、非字面量路径、不相容 key、已知非 Map 中间层或具体 leaf 家族替换
不获得这一前缀证明。已有完整 Map 路径 lowering 保持不变，Struct 禁止路径遍历
的边界保留。receiver 所有的泛型 key 得以保留；空 update-in 返回 callback 输出，
不能借与 receiver 相同的变量名伪造类型归属。

公开 schema、API 名称、运行时语义与 source fix 均不改变，没有新增诊断、checker、
顶层命令或平行 registry。Dynamic 子节点仍在具体消费边界接受检查。

## 验证与限制

五项定义 :tests 覆盖 root/相邻字段、嵌套 Map 前缀、独立 Tag/String/Number 泛型 key、
receiver/key/value 单次按顺序求值和 update-in。现有 CLI post-lowering harness
覆盖独立 root/generic 返回证明，以及空/未知 path、错误/未知 key、具体 leaf/前缀
家族变化、已知 scalar 中间层、开放 root 和空 update-in 的拒绝。

最小 Snapshot 与实际 Rebase Hell repo updater 在候选证明通过；消费者全量及正式
配对发布仍须单独完成，不能把标同版本号的本地候选冒充发布版。

native/JS 五项回放通过。实际 WASM/WASI 尝试仍报既有 E_WASM_NIL_TYPE_EVIDENCE
及 Ref 不支持，按原统一 runner 精确记录排除；没有放宽 backend 检查或宣称已支持
开放 Map 遍历。现有完整 typed-path backend 回归仍保留。
