# 拒绝带实参的 schema alias

## Review 发现

schema alias 本身不声明泛型，但 runtime fallback 最初忽略了 `TypeRef` 携带的 type arguments。这样手写的 `Items<String>` 可能被展开成无参 `Items` 的底层 schema，造成静态与 runtime 规则再次偏离。

## 调整

- 名义 Struct/Enum 仍先按既有逻辑处理 applied arguments。
- 只有 type arguments 为空时才进入 schema alias fallback。
- 回归测试同时证明无参 `Items` 接受 List，而带实参的 `Items<String>` 拒绝同一个值。
- 恢复与 #911 无关的 Dynamic 提示文档行，把后续统一清理留给 analyzer 收敛任务。
