# WASM Enum 方法保留 Number 范围语义

## 现象与选择

Enum 的 nominal Contains 实现按 Number 范围检查索引：非负且小于包含 tag 的元素数时
返回 true。修复 core 的重复来源后，普通 `.contains?` 调用可以通过检查，但 WASM 的现有
方法分派只处理 List、Map、Set，Enum 落入 false 分支；附带断言随后 trap。

在原方法分派里补齐 Enum 分支，读取已有 payload count 并加一，不新增入口或调用 native
替代方法。索引保留 f64 比较，不转整数：分数、负零、无穷与 NaN 都沿用 native 合同。
原分派先求值 receiver 和参数，再检查种类，补分支不重复求值或改变已有分支顺序。

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
