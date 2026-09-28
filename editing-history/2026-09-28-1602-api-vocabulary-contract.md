# API 命名契约：先明确命题，再迁移拼写

关联 [#1452](https://github.com/calcit-lang/calcit/issues/1452)，在已通过 main Test 与 Push on main 的 `981cd44ee9509cb75ca10034be2ff517aca2f4df` 上继续。#1453 的 Unicode 搜索修复已经独立交付；本次不批量重命名、不移除兼容入口、不改变 runtime。

## 契约范围

扩充现有 `docs/features/api-roles.md`，不建立第二份长期 API registry。区分“当前保留”“目标尚未实现”“内部角色”“暂缓及原因”，按谓词、集合、转换/解析、效果与内部实现给出签名、失败/返回行为、迁移边界及对应 issue。保留 PascalCase 类型、kebab-case 值名、Bool 问号、名义 tag variant 与 Lisp 结构；不照搬 Rust 所有权、iterator 或字节索引。

明确选择成员查询保留 `.includes?`，索引/键/值/字段拆成具体命题，不原地改变 `.contains?`。集合长度首选 `.len`；seeded fold、intersperse、join-string、distinct-values 等为下一批目标，不作为当前已可调用的方法展示。Map/Set insert/remove 仍返回新集合，不引入原地变更。带所有权暗示的 as-/into- 不新增，显式 checked refinement 与 JsNullish 包装保留源→目标箭头并区分验证强度。`!` 指明动作/状态/资源操作，不表示一切不纯、抛错或宏；暂缓项有具体理由，不替模块推断 ABI。

每批复用既有 fix/preset 与 revision/source origin，先保证类型证据和用户方法测试，再迁移消费者；未知 macro、函数值、遮蔽和自定义 trait 必须有证明或人工审阅。旧入口退场需实现解耦、至少一个正式发布窗口和消费者证据，不因到达版本号自动删除。

## 源码和消费者证据

核对 core metadata/method table、primitive 注册、native/JS/WASM 源码和实时 query type，不仅按前缀列清单。List<Number> 的 `.append/.foldl` 查询为 open，但严格 eval 成功；`.reduce` 则 proven。因此先补查询证据，不把 open 宣称为实际不可用，也不以 native call 替换方法测试。

只读查看 Respo 本地 `253d129dfbf81c000ee4a788e9d30ce3493e255f`（main ahead 2，未切换/修改），HTML 路径 `respo.render.html/element->string` 使用 `some?/turn-string/join-str`，事件键查询使用 contains?。js-ffi 的本地升级分支 `abcd40cbcc57d19fcca932aa3c0291a08f7466fd` 保持不动，确认 write-text!/set-timeout! 等边界。calcit.std 核对远端 main `615006542181b0afba49484551e301d03bff0f9b`，避免旧本地 checkout 冒充最新模块。这里只记录命名消费证据，不宣称消费者完成了尚未实现的迁移。

## 发现的前置正确性问题

1. native round? 用 EPSILON 小数容差，对 ±1e-16 为 true，对 Infinity 为 false；JS 舍入比较恰好相反。WASM 源码使用 floor 比较，数值边界尚未在本次逐个执行。integer? 的目标为有限且确实无小数部分，需要先修语义并记录兼容边界，不能仅改名，更不能盲改共享 is_integer 的全部调用者。
2. 新增 21 条 Calcit 命题断言在 native 和生成 JS 中全部通过；同一 AST 的实际 WASM 导出中，some? false 与 some? 0 两条失败，其余 19 条通过。nil? 当前按 f64 零判等的 lowering 是排查入口。不要把错误写进预期、换 primitive 或宣称三端已经一致。

两类复现与处理顺序已同步到 [#1454](https://github.com/calcit-lang/calcit/issues/1454#issuecomment-5865921356)，属于下一批谓词工作，本 PR 不夹带 backend 修复。权威反例保存在 `some?` / `contains?` 的 definition `:tests`；新增命题标签只用于选中这两个测试，不是新的统计/检测系统。

## 文档发现与验证

新增规范沿用 `docs read api-roles.md <heading>` 和 `query`；按家族标题缩小读取，不增加命令。测试发现默认 guidebook 来自 `~/.config/calcit/docs` 的安装副本，不随重新编译 CLI 更新，因此首次读取新章节失败。已在文档说明这个来源；Agent smoke 用临时 module 装载本次 checkout 的文档，经相同阅读器验证分段与“目标未实现”的警示，不改用户 guidebook、不重定义 HOME，也不依赖网络。

验证包括：两个新 definition 测试；同一 AST 的 native/JS 运行及 WASM 缺陷定位；API 角色 7/7、query 文档 1/1 可执行代码块；文档格式检查；`cargo fmt --all --check`、`cargo clippy -- -D warnings`、完整 `cargo test`；Agent interface 36/36 及文档读取回归。Agent 查询记录例如 type methods 177.90 ms / 6865 bytes，Option context 454.21 ms / 5621 bytes（当前机器一次 smoke，不是性能承诺）。全量集成最终结果以本 PR 的验证记录为准；额外 WASM 反例明确仍待 #1454，不与现有 CI 通过混淆。

隔离 guidebook 的修正后，完整 `yarn check-all` 通过，含 323 项 core definition tests、既有 native/JS/WASM 路径与 Agent smoke。检查通过不抹去上述两条主动扩展的 WASM 反例。
