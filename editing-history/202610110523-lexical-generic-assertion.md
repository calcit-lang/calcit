# 空集合证明与词法泛型 assertion

真实 Calcium/Reel 组合审计在修复名义 callback owner 后，暴露了两个阶段的问题。
表达式证明一看到泛型就退出，连已知空集合也不能证明其集合种类；assertion 的检查与
推断又把外层 K/V 解析为短名 TypeRef，使 fold 的初始值与 callback 的变量失去关联。

表达式证明现允许已知空 List、Map、Set 满足同种集合的泛型契约，不给空值绑定成员
变量。检查和推断使用词法 scope 的已有泛型名解析目标。断言的独立证明拒绝重新绑定
外层变量；最初的非空泛型反例暴露了 prove_with_bindings 将 T 绑定为 Number 的漏洞。
同名变量可保持既有关系，开放输入仍要边界证据。没有扩大类型、插入 unsafe 或权限。

语义 fixture 用 definition-attached tests 验证三种空集合及 typed fold 的实际回放。
内部 Rust 单测直接检查 TypeVar 的证明关系，覆盖错误集合种类、非空、Dynamic 和
裸 T；该测试针对证明引擎的内部表示，附属 fixture 与真实下游承担公共语义验证。
已有 known-assertion runner 检查非空泛型输入和开放 List 输入被拒绝，Snapshot 不变。
所有 fixture Snapshot 均经官方 CLI transaction dry-run、scoped revision 守卫写入。

最终源码的 core native 610/610、JS 609/609（1 项既有 exclusion）通过；API 基线不变。
四项新 fixture 在 native/JS 各 4/4 通过。完整 Rust 与 known-assertion 最终回放已启动，
结果需要其终态确认；完整 yarn check-all 尚未执行，不能宣称完整集成门禁通过。

Calcium 两个既有 Unit callback 显式返回 Unit，正式 alpha.29 client 44/44、重新生成
JS 的原子 patch 和真实双客户端 Kanban 回归通过，独立热/冷存储 1060/752 字节，
服务 SIGINT 退出 0。显式 stable 0.29.0 的组合源码探针只剩 core pairs-map 一项，
没有把未发布 core/Reel 源码探针混作正式依赖验收。milestone 的其他要求继续保留。
