# 保留 match 的显式开放 payload

## 问题

#1762 的最小复现在 alpha.2、alpha.3 及 #1763 候选上都能通过静态检查，但输入错误值后由 native Struct 构造器在运行时拒绝。`Map<String, Dynamic>` 查询得到 `Option<Dynamic>`，match 后把 payload 或 nil 绑定到局部变量，再用于具体 nominal 可空字段。

调试确认局部变量被错误推断为 `Nil`，并非构造器把 Dynamic 当成了合法的具体类型。根因是 match 构建 Enum 泛型绑定时跳过显式 Dynamic，使 payload 留下未绑定变量；后续分支兼容合并把它与 nil 合并成了 Nil。

## 方案与边界

所有已经应用的 Enum 泛型参数都进入既有替换表，包含 Dynamic。显式开放的 payload 不应退化为待推断变量，也不应从 Enum 的 where 条件凭空取得 trait 证据。未应用的变量、已知具体参数和原有 where 回退逻辑保持原状。

不新增字段检查器、诊断或命令。保留 Dynamic 后，现有分支合并和方向性字段证明自然拒绝未经检查的值；正常传递、保存和通过 `try-decode-map-as` 收窄仍允许。没有改变可变 Ref 策略。

## 验证

- 在既有 `def-value-schema.cirru` 中增加 4 个 definition `:tests`，覆盖开放值保存、Map/Option/Result 的合法解码、错误 payload 和缺值。
- 扩展同一个 `check-known-assertion.mjs` 回放段落，新增 6 个 native/JS 拒绝用例，覆盖 Map 两种分支顺序、Option、Result 两种 payload 槽及局部别名；断言源码不变、拒绝时不生成 JS 应用产物。
- #1763 候选在 open-map-match 负例仍错误通过；本修复拒绝。原 nullable 测试保持可用。
- 本地 no-WASM 构建、481 个 core 测试及 native/JS 定向验证通过；完整 WASM/WASI 与 Rust 门禁由 PR Actions 验证。

关联 #1529、#1553、#1761、#1762。
