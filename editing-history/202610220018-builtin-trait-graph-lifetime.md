# 内建 trait 注册与生成图的生命周期

## 问题与决策

同一 realm 载入两份生成的 core 时，两份 trait 对象具有独立的 nominal 身份，但共享 runtime 的内建实现表原先被后一次注册整体覆盖。前一图的显式调用仍携带原 trait 对象，因而无法在新表中找到自己的实现。

保留这层对象身份，不按短名或 qualified source 名字合并 trait。原注册入口继续更新普通动态方法使用的最新表；另用 WeakMap 以实际 trait 对象为键，记录其每种接收者的已注册实现表。WeakMap 不额外强持有已经不可达的 trait 代际，不给热路径复制整张表。

显式 trait 调用和 assert-traits 共用现有 lookup。依赖 trait 逐个按自己的对象查找，因为应用定义的子 trait 未必直接注册为内建 trait。部分注册只更新给出的接收者类型；同一对象重新注册相应类型时更新该项。

## 保持的边界

- 不改变 trait 判等、名义值/定义判等、编译器证明或用户签名，不引入新的公开入口。
- 普通动态方法仍选最新表；相同名字的新 trait 对象不能冒充旧对象。
- 保留完整原表供使用阶段校验，不将非法表的错误提前到注册阶段；非法列表仍在使用时报错，重复 nominal impl 不按顺序挑选。
- 不更改其他 core 注册器的重载合同，不承诺跨版本 runtime 混用或跨 realm 的对象兼容。

## 验证方式

沿用 JS artifact CLI harness：正常生成两份 core，确认代码字节相同，各自保留相对依赖，在两个独立 Node 回放中分别先加载 A/B。验证旧、新图的 ToString 和 assert-traits、应用子 trait 的内建依赖，以及同名独立 trait 拒绝与重复 impl 错误。

沿用 JS runtime identity 检查，覆盖同对象重新注册、不同接收者的部分注册、普通方法的最新表行为及非法实现列表的错误时机。模块载入生命周期不能由普通 Calcit definition 测试构造，因此只将此宿主边界放在已有 harness；语言语义继续由原 core definition tests 在 native、JS 回放。确保 JS 运行器实际解析到本次编译的 runtime，不借用其他 worktree 的包。

完整 core 回放通过 native 689 项、JS 688 项，保留原有 1 项 JS 排除。扩大到全部 26 项旧 trait 测试时，native 有 7 项、JS 有 9 项在编译阶段被拒绝，不能记为全量通过。使用精确 main 的原 runtime 对照后，52 个后端结果的状态、错误编号、失败文本及 trace 均相同；编译期拒绝与旧运行期断言的阶段错位另由 #2259 整理。本修复不删除这些断言、不新增排除，也不将它们的编译失败当作运行期语义已验证。

```bash
yarn compile
node scripts/check-js-runtime-identity.mjs
cargo test --test js_artifact_output_cli --test js_namespace_import_cli --test js_global_shadow_cli
node scripts/run-core-tests.mjs --backend native,js
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

旧 trait 测试的诊断对照另执行 `node scripts/run-core-tests.mjs --snapshot calcit/test-traits.cirru --backend native,js`；上述非绿色结果保留在问题记录中，不属于通过项。
