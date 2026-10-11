# 键值对转换与嵌套字面量 spread 的真实证据

## 来源与取舍

Respo #194 的 Calcium/Reel 完整 strict 回归暴露 `pairs-map` 的旧合同：输入只有 `List<P>`，返回却承诺任意 `Map<K,V>`，K/V 没有输入证据。保留原输入范围和逐项两元素 List 校验，将开放返回改为 `Map<Dynamic,Dynamic>`。名义值的异构键值对应由调用方通过 typed fold/assoc 保留各位置的类型。

已知 `List<List<T>>` 的两个位置都具有 T 证据，现有 receiver-specialized 合同可返回 `Map<T,T>`，包括调用方的词法泛型 T。异构或错误成员不能凭声明进入闭合消费者。运行时空输入、nil、重复键最后值覆盖，以及损坏 pair 的断言消息保持不变；未引入新的 Pair API。

Calcit #1961 的两个失败 Actions 均在已知返回检查中拒绝嵌套字面量 spread。原实现只展开一层并明确拒绝内层 ArgSpread；新实现按源顺序迭代展开字面量 List 构造器，包括规范化的 CallSpread 包装。运行时 List 继续缺少固定参数个数证明，错误成员继续拒绝。此修复只恢复预处理证据，不代表该 PR 的新 WASM lowering 已交付。

## 测试与边界

- core `pairs-map` 附带四项测试覆盖普通转换、同类型成员、空/重复/nil 和损坏 pair；native/JS 4/4，WASM/WASI 3/3。损坏 pair 测试使用 try，两个共享 WASM 后端显式排除该项，原因与实际 codegen 错误一致。
- definition fixture 回放 generic/closed homogeneous pairs，并拒绝异构或错误成员进入 Number Map 消费者；嵌套 spread 验证求值顺序、native/JS 等值，并拒绝开放参数及错误成员。已知 assertion 脚本校验审计不修改 Snapshot。
- core 全量 native 613/613、JS 612/612，保留已有一项 JS 排除；API 合同基线未变。Rust 1676 passed、0 failed、1 ignored；fmt/clippy 通过。
- Calcium 的三处异构 pair 路径分别改为直接 Column Map、typed filter-map-kv 和 typed fold/assoc。在正式 alpha.29 下三个 definition 测试 native/JS 均为 3/3。
- 回放工具在临时 Snapshot 通过官方 config 命令显式设置 native 模式，修正页面 Snapshot 默认 JS 模式导致 native 只生成文件、没有执行测试的问题。零标记检测曾正确拒绝该假通过；修复后真实 native 与 JS 回放均通过。

首次完整 `yarn check-all` 的已知 assertion、Agent interface 64/64、API 门禁、core unit 585/585 与全量 native 613/JS 612/WASM 380 回放均通过，随后被过期的 Dynamic 分类资料阻断。生成器增加 pairs-map 开放键值及 update receiver-specialized callback 的四处分类，生成资料为 296 个位置；原待迁移位置数仍为四。生成文档以 gitattributes 标记 generated，并保持可审阅 diff。分类门禁与其后的 native/JS/IR/WASM、结构字段、literal path 和 typed-method-rem 命令均通过。更新资料后，使用固定 CI CLI、`CALCIT_LINT_CORE=1` 再次执行完整 `yarn check-all`，最终退出 0，以此作为整条门禁的成功证据。

Calcium 后续正式工具链的安全构造器/do 迁移通过原 107 项测试、公开 browser 157/native 239 检查、SSR 与真实双客户端网络回归；未发布 core/Reel 源码组合的完整 strict workflow 已 passed，保留人工边界清单。Timegrass 使用已准备的 Alerts alpha.4 源码后清除两个模块证明错误，后续真实 dispatch 边界正在迁移；alpha.4 尚未获发布授权。

日志保留于忽略的 `.calcit/pairs-map-final-*`、`.calcit/pairs-map-check-all-remaining.log` 与下游 `.calcit/`。#194 的 inventory、渲染节点类型方案、两下游完整回归与协作交付仍需继续。

## 后续同步与下游结果

Timegrass 的 renderer 开放入口现先经真实 decoder 验证 Op，再进入闭合业务 dispatch；
watch callback 显式保留两个独立泛型参数。正式 alpha.29 通过 client 18/server 94、
公开 browser 130/native 134、JS 生成与真实双客户端网络回归。安全构造器迁移后，
同一 Snapshot 使用未发布 core/Reel/Alerts alpha.4 的完整 strict workflow passed，
diagnostics 0、安全迁移 0；人工类型与 FFI 边界清单仍保留。正式依赖未改，不作为
正式工具链 receipt，也未重跑最新浏览器 DOM。

最新读取的 #1961 已合并，head 529eebb7 的 Actions 全绿，main 为 a20ffcf1。
本候选已通过 Git 正常三路合并同步至该 main，合并提交 5f50c72e；无冲突。
对同步后的候选重新执行 fmt/clippy、完整 Rust 与 CALCIT_LINT_CORE=1 check-all，
最终结果将在同段追加；此前 5fa7829e 的完整门禁通过不能冒充同步后的结果。

同步后的 fmt/clippy 通过。首次全量 Rust 在两个 component HTTP fixture 绑定本机
端口时被沙箱拒绝（Operation not permitted），停止于该 suite；原日志保留。
随后允许本机网络监听后重跑完整 Rust，再继续构建与完整 check-all，不能把首次
沙箱失败算成通过，也不通过排除网络测试绕过该门禁。

同步至 main a20ffcf1 的代码候选 5f50c72e 最终验证完成：fmt/clippy 通过，
允许本机网络监听后的完整 Rust 为 26 个 suite、1680 passed、0 failed、1 ignored。
新构建的 CI CLI 执行完整 CALCIT_LINT_CORE=1 yarn check-all，最终退出 0。
其中 unit tag 588、完整定义 native 617/JS 616/WASM 385，通过数不包括
JS 1 项和 WASM 232 项明确排除；API、Agent interface 与 Dynamic 分类门禁通过。

用该新 CLI 对同一 canonical Snapshot 重新执行两个下游源码组合的完整 strict，
Calcium revision md5:e81d2e900c80c72bf2ded1e99c2adf2c、Timegrass revision
md5:fa58e05bbe58e43627beb2d275a510cf 均 status passed、diagnostics 0、安全建议 0。
Calcium 保留 44 项源码审查、106 项类型发现、62 项 FFI 边界、18 项 schema 候选；
Timegrass 对应 1、165、59、14。源码组合和正式版本的边界如前，未更新正式依赖。

最终日志为忽略目录 main-sync-clippy.log、main-sync-rust-network.log、
main-sync-build.log、main-sync-check-all.log，以及下游 workflow-main-sync.json。
功能分支尚未推送或建立 PR；双语正文已准备，待明确外部写入授权。
