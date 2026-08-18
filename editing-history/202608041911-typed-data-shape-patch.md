# Typed data shapes and nominal patch foundation

## 背景

严格 EDN 解码已经能在反序列化边界恢复闭合静态类型，但其类型图原先仍属于解码器私有实现。struct 更新和 Recollect diff/patch 也缺少同一套可校验的名义数据约束，容易在新的数据边界重新引入 Dynamic。

## 本次实现

- 将解码器类型图提取为 backend-neutral 的 `DataShapeGraph`，保留 ABI version、稳定 fingerprint、名义类型路径和闭合子节点关系。
- 为 `DataShapeGraph` 增加运行时值验证，覆盖标量、容器、ref、struct 和 enum；struct/enum 必须匹配名义定义、字段或 variant 及递归 payload 类型。
- 严格 EDN Native/JS 路径复用同一 data shape；JS graph 同时携带 ABI version 和 fingerprint，并拒绝畸形 graph 元数据。
- 新增内部 `DataPatch` 第一阶段执行器，支持 keep、replace、struct 字段 patch 和 enum payload patch。patch 与 shape 的 ABI/fingerprint 必须一致，且应用后继续保持原有 struct/enum 名义对象。
- struct indexed 更新同时校验编译期 index 与 field tag，并拒绝负数、非整数及越界索引，避免 schema 演化后静默更新相邻字段。
- 类型推断对 record assoc/with 的普通与 indexed 形式保留精确 receiver 类型，包括泛型实参；预处理同步检查更新值类型。

## Recollect 联动

- `change-op` 的真实构造迁移到名义 enum，避免 diff 结果退化为无约束 tuple。
- `:map-splice` 的 removed payload 修正为 Set，与 diff 产物及 patch 消费端一致。
- collection diff 策略暂时保留在 Recollect；后续应按 `DataShapeNode` 绑定 list/map/set 的 typed patch 策略，而不是把 Dynamic collection payload 直接搬进编译器核心。

## 验证

- `cargo fmt -- --check`
- `cargo clippy -- -D warnings`
- `cargo test -- --test-threads=1`：513 项通过
- `yarn compile`
- `yarn check-all`
- `yarn check-agent-interface`：12/12 通过
- Recollect check-only 零告警，Native、JavaScript 和阻断性 WASM 回归通过
