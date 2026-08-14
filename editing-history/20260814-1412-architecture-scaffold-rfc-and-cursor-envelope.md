# 功能架构脚手架 RFC 与 cursor envelope 兼容性

## 背景

单 definition 的 `cr edit def` 无法直接承载一个功能的调用结构、接口约束、已有节点复用和批量生成计划。未来多个 Agent 拆分实现时，还需要在不承诺同 Snapshot 并发写安全的前提下，确保 cursor sidecar 不会丢弃其他 cursor user 的位置数据。

## 本次结论

- 新增 `RFCs/08-14-architecture-scaffold-rfc.md`，建议以 `cr edit scaffold` 接收 Cirru EDN desired-state overlay；canonical model 是 Symbol FQN 到 declaration 的平面 map 加 typed anonymous-enum edge set（如 `:: :call from to`），tree 只作为展示或未来输入语法糖。
- architecture declaration 包含 mode、kind、doc、当前 canonical schema、函数参数和可选 code/examples；已有 definition 默认复用，具体 schema/kind 冲突拒绝整次 apply。旧示例中的 quoted `:: 'Fn` 已修正为 unquoted `:: :fn`，只有 code AST 使用 quote。
- 已有 definition 不会从 scaffold graph 中消失：planner 同时展示 existing/planned doc、schema、kind 和字段级 diff；apply 不覆盖现有内容，差异通过 info/warning/conflict diagnostics 报告。
- 函数 `:params` 属于程序内部元数据，architecture Cirru EDN 使用 Symbol（如 `'order`），parser 不把 String/Tag 隐式转换成参数名。
- scaffold stub 不再用普通 `raise`：新增 `08-14-todo-placeholder-rfc.md`，设计 compiler-known `todo!`、internal Never、`W_TODO` 和各后端的 diverging 行为；scaffold apply 可创建 stub，但 TODO 完成门禁仍失败。
- scaffold 与 TODO 的规范机器结果改用 Cirru EDN，保留 Symbol/Tag/Set/Quote/schema 的数据身份；JSON 只作为既有 Agent、`jq` 和 LSP/MCP 工具的兼容 renderer。
- 多 Agent 第一版采用“并行产出 definition 实现、parent/coordinator 串行写回 Snapshot”；CLI 只输出带 target/write-set/schema/doc/planned edges 的 work item，不启动 Agent，也不把 call graph 当成硬任务依赖。
- reconciliation 区分 `create`、`reuse-pending`、`reuse-complete` 和 `external`；带 scaffold/TODO 的已有节点会在重复 dry-run 时继续产生稳定 work item，保证中断后可恢复。
- definition-level semantic patch、`ensure-import` 和 SCC/batch 建议留到流程走通后；第一版继续复用 Snapshot revision、现有 edit/transaction 与 staged atomic write。
- 第一版不暴露只有单一有效值的 `--stub`/`--existing` 策略开关，直接固定为 compatible reuse、hard conflict reject 和 TODO stub。
- cursor 身份仍为 `--cursor-user` > `CALCIT_CURSOR_USER` > `default`，但 architecture 不再把 work item 绑定到 cursor user。未来多 user 推荐 `.calcit/cursors/<user>.cirru`，source mutation 只立即迁移当前 user，其他 user 下次访问时 lazy revalidate。当前新建 v4 sidecar 的默认 cursor key 先从 `main` 改为 `default`。
- scaffold 只生成普通 `CodeEntry` stub，不增加运行时架构元数据；后续实现和 drift 检查继续复用现有 call graph、query、tree 和 cursor。
- cursor v4 文件本来已有 `:active` 与 `:cursors` map，但 Rust 内存模型只保留 active state，任何保存都会把 map 收缩为固定 `:main`。
- `CursorDocument` 现在保存实际 `active_name` 和其他 cursor state，序列化时合并写回，因此可无损 round-trip named cursors；CLI 行为仍只作用于 active entry。
- architecture 文件实现期间可约定放在 `docs/architectures/<feature>.cirru` 并用 normalized content hash 形成 plan-id；派生的 status/work items/diagnostics 不回写 plan。`.calcit/` 只承载本地临时 cursor 状态。
- 本轮没有实现 cursor user 选择、per-user 文件、semantic patch、lease、心跳或并发锁；这些都不阻塞 architecture → scaffold → work items → 串行整合的第一版闭环。

## 验证

- `cargo fmt --all -- --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface`（12/12）
- `yarn check-all`
- `cargo test --bin cr cli_handlers::cursor::tests`（19/19，包含 `default` key 与 named cursor round-trip）
- 用 `cr cirru parse-edn` 验证 RFC 中 architecture、machine result 与 work item 示例；带空格 doc 使用双引号保护 pipe string。
- architecture result、TODO diagnostic、通用 machine envelope 与 Definition Descriptor 的 EDN 示例均通过 `cr cirru parse-edn`。

新增/扩展的 cursor round-trip 测试覆盖非默认 active 名称和非 active cursor 条目的保存与恢复。

## Scaffold 与 `todo!` 基础实现进度

- 新增 `cr edit scaffold`；接受平面 architecture Cirru EDN，输出 human、canonical EDN 或 JSON compatibility projection。
- parser 强制 FQN/params 使用 Symbol，edge 使用匿名 enum（`:: :call from to` / `:: :type from to`），拒绝误用异构 list 的 edge。
- planner 验证 roots/edge endpoint、schema 与 namespace，reconcile 现有 definition，并输出 create/reuse-pending/reuse-complete/external、预览 operation 和带 plan/base revision/write-set 的 work item。
- `--dry-run` 保持只读；apply 在 staged Snapshot 中一次创建全部缺失 `:ensure` definition，function 生成 `todo!` stub，写入 doc/schema/`:scaffold`，复核原 revision 后 atomic rename，绝不覆盖已有 definition。apply 成功后触发已有 cursor 后置校验。
- 新增 compiler-known `todo!` proc：native effect 中断，preprocessor 产生 `W_TODO`，JS/WASM codegen 均显式中断。完整 Never/branch/generic inference、definition-level patch 和 external dependency/core lookup 仍待后续实现。
