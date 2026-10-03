# 项目级结构改写与严格暂存门禁

对应 #1695，延续迁移工具收敛方向：项目自己的旧 helper / 调用形状不进入固定 API rule 或 preset 表。
在既有 `fix` 入口增加显式 pattern/template，复用 query 的结构匹配、fix envelope、源码 fingerprint、revision/VCS/工具链守卫和原子事务。

## 审阅与语义边界

匹配按原 AST 进行；同名变量要求完整子树相等，模板仅能引用已绑定变量。
对照模式与模板的变量出现次数，拒绝丢弃或增加次数，包括模式中本来就重复的变量。
出现次数相同并不证明求值顺序、binding 或失败语义，全部候选保留 requires-review，不发布新的编译器诊断编号。
显式 apply 是操作者采用已审阅模板的授权，不把语法候选提升为 machine-applicable。

嵌套原始命中由内向外合成为每个源码区域的一次替换，不重扫新生成代码；无法合成的重复变量/结构关系整批拒绝。
quote/quasiquote（含限定名）按语法保守保护；tests/examples 使用各自的编辑接口保留标签和顺序。
祖先模板也必须原样保留已有 quoted 子树及出现次数，否则跳过该候选；仍遍历未引用子节点，完整搬运 quoted 参数的替换不受影响。
schema、doc、namespace imports 不作为模式扫描区域，也不自动补齐业务类型。

## 验证与旧源码

预览只解析和扫描 source，不要求旧表达式通过类型证明，也不执行其主体。
旧源码可能恰恰因为迁移前的调用不存在而失败，因此不将旧表达式的严格编译成功作为 apply 的前提。
应用必须在暂存 Snapshot 严格预处理所选 scope；附带区域通过已有 preprocess 的匿名函数包装检查，不注册第二套分析程序，也不执行断言主体。
任何错误或 warning 都阻止整批发布。宏的已声明日志仍可在 stderr 观察，stdout 保持一个结构化文档。
该门禁不能代替原 Calcit 断言和业务目标的 native/JS/WASM 回放。

内部 AST/绑定与事务协议用 Rust 测试；观察值写在通过 Calcit CLI 创建的附带断言中。
已覆盖跨 namespace、嵌套命中、quote、测试/示例、模板变量错误、revision/VCS 拒绝、代码或仅附带区域类型错误回滚，以及旧非法调用修复后的严格暂存校验。
Respo 的既有迁移副本已验证 `some?` 到 `non-nil?` 的显式事务，原 HTML 附带测试及同一断言的生成 JS 回放通过，用户原项目未改动。
完整 all-features Cargo 回归、Clippy、TS 编译、两份文档代码块与 Agent CLI 协议检查已通过。
临时 node_modules 的运行时身份和共享 target 的包解析链接修正后，全量 `yarn check-all` 成功退出；祖先 quote 保护另有 AST/CLI 回归。
最新 HEAD CI/review、合并后 main 门禁仍须完成。
这些使用未发布候选的回放不作为正式版本的消费者兼容性证明。
