# 短路 fold 累积值证明

## 原因

`foldl-shortcut` 的 proc signature 保留 Dynamic 返回。即使实际 reducer 的控制位和所有 payload 都是具体值，完整返回证明仍失败；checked decoder 的 core `every?` 因而也暴露出证明债务。

## 方案

复用既有 collection fold 参数上下文，为 reducer 提供累积类型和集合元素类型。短路 fold 额外检查 default；返回推导检查实际预处理 reducer 的匿名 Enum 控制位及 payload，而不把宽泛 Enum 返回声明当作槽位证明。控制流各活跃分支必须证明同一累积契约，直接 raise 可以作为无返回分支。

不引入新的表层类型或 every? 特例，不按 initial/default 猜未知 callback 返回。开放 callback、未知 payload 和无法读取实现的值保留未证明。命名函数只读取已有 compiled source，不为证明求值 thunk。

自查发现 caller 的 Number 参数与 reducer 内 Dynamic let 同名时，旧实现会错误借用 caller 证据。修复统一的 let/match 词法推导，Dynamic binder 也必须遮蔽外层同名证据；callback 参数独立绑定，具名实现不继承 caller scope。加入 let/match 同名负例，实际错误返回 String 不能再被证明为 Number。

Review 补充发现 core-let 的最后一个局部变量仍直接读取 inline type，而没有读取刚建立的 scope。内部 lowering 最小回归在修复前得到 `Some(Dynamic)` 而不是 initializer 的 `Some(Number)`；尾部改用已有 `resolve_type_value`，同时验证 String initializer 与未知 initializer 遮蔽外层 Number。该精确 Rust 回归只验证用户源码通常已经附上类型的内部表示，表层与跨后端行为仍复用 Calcit `:tests`。既有 runner 另加入外部裸 Fn reducer 的无写回负例，防止只凭 initial/default 授予具体返回类型。

## 验证

语义放在 core definition `:tests`，既有 assertion runner 从这些源码表达式回放 native/JS 和显式 proof。负例覆盖控制位、payload、default、开放内容及混合分支，preview 保持源文件字节不变。WASM 能力和 core decoder 的剩余义务按实际结果记录，不把普通执行成功写成完整证明成功。
