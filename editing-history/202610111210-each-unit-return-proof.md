# each 的独立 Unit 返回证明

## 下游阻塞与修改

Timegrass 在正式 CLI/procs alpha.29、Respo alpha.15、Message 0.0.32 组合中执行完整
strict 工作流，`calcit.core/each` 出现 `E_FN_RETURN_UNPROVEN`：公开合同返回 Unit，
但开放集合的 foldl 返回值推导为 Dynamic。此次修改保持原来的迭代、回调和公开签名，
在 foldl 完成后显式返回 `&unit`。异常仍从 foldl 传播，不以 assertion 或 unsafe
声称开放结果已获得证明。

沿用 each 的定义附带测试（回调可返回非 Unit），新增空集合不调用 callback 的用例。
通过既有跨后端运行器回放，不增加独立脚本或改变排除列表。

## 已验证的范围

- 官方 alpha.29 CLI 对 main 的旧 each 报告返回缺证据；对修改后的同一 Snapshot
  使用相同 concrete-return-proof-v1 查询，诊断及建议均为空。
- `cargo fmt --check`、TypeScript 编译、默认 feature 的
  `cargo clippy --locked --profile ci --bin calcit -- -D warnings` 通过。
- 当前 main alpha.30 加此次 core 源码的 native-only 和 WASM-enabled CLI 构建通过。
- 原生 unit tag：577/577；完整 core 定义测试：native 605/605、JS 604/604，
  仅沿用既有的一项 JS 排除。
- each 的两项测试：native、JS、WASM 各 2/2，无排除。
- 既有 core API contract 检查通过，公开 schema/arity 与方法 specialization 未变化。

Timegrass 原候选固定 alpha.29，alpha.30 候选 CLI 的完整 strict 命令被版本守卫拒绝，
没有修改应用 Snapshot。另建忽略目录的隔离副本，仅显式把 CLI 声明调整为 alpha.30
用于候选源码审计，原有业务源码与模块链接保持；这不是正式 alpha.30 工具链 receipt
或 JS runtime 验收。该副本的完整 strict 剩余三项诊断：Alerts 事件/样式与 core update；
each 诊断消失。仍有一项 twig-members constructor 源码审查和 165 项类型发现；main
alpha.30 与正式 alpha.29 存在其他变化，不能把所有报告差异归于此次修改。

报告、构建产物和 fixture 位于 `.calcit`、target 或临时目录，不入库。功能分支尚未
发布；下游正式 alpha.29 的完整 strict 仍未通过。

首次 clippy 指定 no-default-features 时，Rust 报两个 unused import 和一个 dead-code；
未修改这些 Rust 路径，默认 feature 检查成功不能代表无默认 feature 配置也零告警。
完整 `cargo test --locked --profile ci` 仍在构建测试依赖，结果待记录；以上测试不替代
该命令及仓库全量 check-all。
