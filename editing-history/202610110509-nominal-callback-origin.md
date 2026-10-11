# 保留 lowering 后的名义 callback 身份

Calcium 的严格源码探针发现，if-let 生成的 callback 将 Session 返回类型写成短名，
随后在 core/macro 作用域解析 hint 时，无法恢复源定义身份。最小 fixture 使用两个
同名 Item，验证合法 callback 保留左侧定义，混用右侧定义仍被 native/JS 拒绝。
内部 Rust 测试检查生成 hint 的 roundtrip，覆盖 struct、enum、泛型参数与无源 owner 的回退。

修复仅在生成 callable metadata 时递归保留 source-backed nominal 的完整 owner。
公开 schema serializer 保持原格式。初次扩大公开 serializer 的方案触发 FsPath API
基线差异，因此收窄到生成 hint；原有 API 基线验证通过，没有改写冻结基线。
未转换的标量复用原 Arc，Dynamic 继续复用单例。

同一实际迁移发现 conj 的 variadic recur 无法独立证明 List 返回值；以 foldl/append
保持原顺序与泛型签名，附属测试覆盖空输入、spread、顺序和原列表不变。
统一 core test runner 回放外部 Snapshot 时保留已安装 modules graph 与版本 manifest，
让真实 Reel 测试复用已有 runner，无需重新安装依赖或复制独立脚本。

最终源码验证：Rust 1665 passed、1 ignored；core native 610/610，JS 609/609，
1 项既有 JS exclusion；conj native/JS/WASM 各 3/3；known-assertion 正反例通过；
原有 API 基线、cargo fmt --check、cargo clippy --profile ci -- -D warnings 通过。
Rust HTTP fixture 在 sandbox 下因端口权限失败，授权的完整重跑成功。
完整 yarn check-all 尚未执行，不能将上述结果写成全量集成门禁通过。

Calcium 与 Reel 的组合源码探针使用未发布的 core/Reel 候选，与正式依赖验收分开。
这些本地结果不代表 milestone 的 DOM、dispatch 方案选择或所有下游验收已完成。
