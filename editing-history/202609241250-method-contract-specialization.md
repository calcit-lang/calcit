# 方法契约状态以接收者实例化结果为准

## 问题

`query type ":: 'Option 'Dynamic"` 曾把 `.unwrap-or` 和 `.and-then` 标成 `proven`，同时展示含 `dynamic` 的参数或返回值。原判定只检查声明 schema 中的 `Dynamic`/`DynFn`，没有检查 `TypeVar` 绑定到接收者类型后的结果。这会让 Agent 误以为开放数据已有精确调用契约。

## 决策

复用原有类型变量替换与动态权重判定，不新增查询规则或类型状态。先实例化方法参数、rest 与返回值，再决定 `proven` 或 `open`；实例化后仍含 `Dynamic`/`DynFn` 时不输出伪精确的签名。不读取开放 payload 的方法仍可保持 `proven`，例如 `Option<Dynamic>.some? -> Bool`。trait/impl 歧义仍是独立的 `ambiguous` 状态。

## 验证与边界

查询回归覆盖 `Option<Dynamic>` 的 `.unwrap-or`、`.and-then` 与 `.some?`，并保留 `Option<Number>`、List、Map、Result 等已有精确签名检查。此修正只影响只读方法契约证据与其 revision，不改变运行或 strict 编译语义；真实 Agent 工具调用效率仍需按 #1302/#1304 验收。
