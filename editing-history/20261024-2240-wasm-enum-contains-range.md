# WASM Enum 方法保留 Number 范围语义

## 现象与选择

Enum 的 nominal Contains 实现按 Number 范围检查索引：非负且小于包含 tag 的元素数时
返回 true。修复 core 的重复来源后，普通 `.contains?` 调用可以通过检查，但 WASM 的现有
方法分派只处理 List、Map、Set，Enum 落入 false 分支；附带断言随后 trap。

在原方法分派里补齐 Enum 分支，读取已有 payload count 并加一，不新增入口或调用 native
替代方法。索引保留 f64 比较，不转整数：分数、负零、无穷与 NaN 都沿用 native 合同。
原分派先求值 receiver 和参数，再检查种类，补分支不重复求值或改变已有分支顺序。

## 擦除前保留 key 类型证据

Contains 的 key 合同是泛型，普通检查仍允许 Enum 接收非 Number，再由运行时报错。
WASM 不能把已擦除的 nil、Bool 当作索引 0/1。分派使用原参数的静态类型：Number 继续
比较范围，明确的非 Number 在 Enum 分支 trap；Map 等原有分支不受这个 Enum guard 影响。
开放 key 复用已有标量证据诊断及调用点特化，先把泛型 K 绑定到实际类型，再生成该分支。
无法证明的开放调用明确拒绝 codegen，不猜 f64 的 0/1，也不改变表层语言合同。

宿主负向测试还确认 JS 比较会隐式转换非法 key，因此共享 core Enum 实现先检查
`number?`，再执行原范围比较；非法 key 明确报错 `Enum .contains? expected a Number index`。
它沿用 native 拒绝非 Number 的语义，并使 JS 在同一边界失败，不再依赖宿主数值转换。

附带测试保留泛型 Number 的零、负零、分数、越界和 NaN。现有宿主检查脚本使用真实泛型
定义及 native/JS/WASM 调用，验证 nil、两种 Bool、String、Tag 被拒绝，而 Number 0 和
1.5 仍成功；WASM 异常由宿主捕获，避免为了测试异常而新增 WASM `try` 或重复运行器。

## 验证与边界

- 在原 core Enum impl 定义的 `:tests` 增加 Number 范围用例，保留已有测试与断言。覆盖 tag、
  payload、无 payload 的 Option、负数、负零、分数、越界、正负无穷和 NaN。
- 使用官方事务、dry-run、scoped revision 和完整查询读回。对整份官方解析结果按 EDN 的
  Set/Map 语义比较，程序 AST 的 List 顺序仍严格保留，只允许新增目标附带测试。
- WASM 与 WASI command 共用这个分派；使用现有跨后端运行器和实际 Wasmtime 验证，不新增
  runner、CI job、诊断编号、Dynamic、unsafe 或放宽检查。
- 原排除项只有在同一测试实际通过后才移除，不能通过改断言或增加排除获得通过。

另有公开 `contains?` 的泛型 fallback 在 `bool? result` 后报错，字面 Bool/Number 正控却正常。
这需要独立检查结果类型证据；本分支不凭方法名承诺所有开放调用必然返回 Bool，也不把这项
公开函数边界写成已经修复。关联 issue/PR 记录实际验证和剩余任务，不以本文代替验收。

名义 trait 调用内联后直接进入数值比较的路径不经过上述运行时方法分派；它的非 Number
拒绝属于另一处类型证据边界，必须独立回归，不能凭这里的 Number 正向测试宣称已覆盖。
