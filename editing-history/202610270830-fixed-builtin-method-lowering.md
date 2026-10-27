# 内建类型的固定方法表与 WASM lowering

## 问题与决策

`&list:find-last-index` 保留原有泛型合同，仅将初始化中的 `count xs` 换成 `.len xs` 并重新构建嵌入 core 后，普通应用的 `.find-last-index` / `.last-index-of` 在 native、JS 通过，在 WASM、WASI 报 `unsupported invoke method in WASM: .len`。

WASM 的参数证据已有 `List<T>`；问题是已有 runtime 方法解析入口只接受拥有实际实现身份证据的名义类型。List 等内建类型的方法表固定，成员类型参数不会改变该表，因此不需要借用名义值的 `exact_impls` 标记。

复用现有内建类型到 core 实现表的映射、方法优先级及 callable lowering。方法解析入口集中处理两种已有证据：内建接收者的固定方法表，或名义接收者实际携带的实现身份。`Nullable` 不选择内层固定表；开放类型也不能凭方法名选择目标。名义值的 `exact_impls` 保持原义，避免把数据合同误认为实际附带实现。

## 兼容边界

只修改 core 反向索引 helper 的一个调用叶子；不改公开 API、schema、原附带测试或 `count` 的可用性。不添加函数名特判、方法白名单、Dynamic 或 unsafe。实际 callable 仍走既有求值与后端能力校验；未知目标或不支持的宿主能力不会因此获准。

## 验证方式

- 比较原 `count`、只改 `.len`、修复方法解析三份实际重新构建的 CLI；普通应用的原两项 Number/String 附带测试在四后端回放，排除“只修改 Snapshot、未更新嵌入 core”的假验证。
- 通过原 core 附带测试检查边界、遍历顺序、短路、不可变输入；原应用附带测试在 native/JS 检查回调错误传播。
- 回放既有 `impl-origin-portable`，验证名义值跨别名、分支、循环和回调时仍使用实际实现。
- WASI CI 增补相同的两项公开方法回归，继续使用统一运行器；native/JS/WASM 已有同源门禁，不新增 runner 或排除项。
- 执行完整 Cargo、Clippy、TS 构建、Agent CLI 与全量集成检查；实际结果记录在关联 PR。

关联：#2311、#1553。
