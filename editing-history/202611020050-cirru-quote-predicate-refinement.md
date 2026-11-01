# 补齐既有 CirruQuote 谓词收窄

Editor 的官方 namespace 视图解码器先检查 `cirru-quote?`，再构造代码字段为 `CirruQuote` 的本地记录。0.32.1 仍将成功分支的输入视为 Dynamic，拒绝这个受检字段；最小复现见 #2521，消费者为 calcit-lang/editor#83。

在既有 `extract_predicate_bindings` 表中补充 `cirru-quote?` 与现有 CirruQuote 类型的对应关系。沿用 bundled-core 来源检查，不认可同名用户函数，也不扩大 Dynamic 的兼容关系。未新增 proof、fix、诊断编号或语言入口。

语义回归放在 `calcit.core/cirru-quote?` 的 `:tests`：以实际代码 quote 构造强类型字段，并断言普通 String 仍被拒绝。复用共享运行器在 native 和 JS 回放。原 Rust 来源检查扩展同一个谓词，覆盖用户函数伪装及 core 被修改的内部条件。

独立 0.32.1 本地候选验证：新测试在原编译器失败、修复后通过；共享 native/JS 通过，core unit 740/740，原 Rust 来源测试通过。Editor 新增七个编解码定义在候选中检查通过，实际 native/Node 元数据往返、单字段修改和非法输入拒绝通过。以上不是官方版本或 Editor 整体迁移已通过的声明。

修复已转到当前 main 的独立工作树。新的测试在 main 的 native/JS 共享回放通过；WASM 实际报出不支持 `&struct-def:new`，因此仅为这个新增测试在现有 WASM/WASI 排除表记录共享后端边界，不改变已有测试选择或支持承诺。

main 的完整 Rust 测试还暴露 #2523：trait member 类型限定测试直接安装/移除共享 namespace，却未取得已有 program-state guard，整套并行运行失败而同一二进制单项通过。只补同一个 guard，保留原断言、生产逻辑及默认并行运行。

稳定版工作树只用于最小复现和候选验证；最终修复按 main PR 流程验收。不从旧 tag 发布未经批准的维护版，不以候选替代下游正式工具链。
