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
完整 `cargo test --locked --profile ci` 已退出 0，全部 Rust 测试套件通过；仓库全量
check-all 尚未运行。该结果对应 each 修改，不包含后续仅在隔离副本中的 update 探针。

## 后续 update 探针

既有 checked-call-contract 已为 Map/List 从接收者提取 value/key 类型；Struct field
specialization 也已存在。开放 update 实现中的 Fn<T> 缺少独立 payload 证据。在忽略
副本 `.calcit/update-proof-probe.cirru` 中，将 fallback callback 合同改为
Fn(Dynamic)->Dynamic，List/Enum 的索引使用 Number 解码，实际命中的 Struct key 使用
Tag 解码，callable-contract-proof-v1 诊断清零，既有 update 测试 native/JS 通过。

这不是正式源码改动，也未证明全部精确调用合同与失败语义不变。下一步必须验证
错误 callback 的静态拒绝、Struct 与 Enum 路径、不存在的 key、错误索引及回调异常，
再决定是否应用到源码；不把探针通过当作完整迁移验收。
