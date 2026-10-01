# 已知导入值的可调用性检查（#1581）

未发布 main 604062ee 上的最小快照通过 Calcit CLI 构造：app.config/site 声明 Map<Tag,String>，app.main/main! 使用正常 namespace alias 调用 config/site :storage-key。已发布 alpha.3 的 --check-only 通过，JS 生成 $app_DOT_config.site(...)，与真实 Proto Shuangpin 的报错路径一致。

根因是 check_callable_type 把所有 Import 无条件视为可调用，绕过既有 infer_type_from_expr 和 is_callable_type。移除 source import 的跳过分支，让已知值复用通用判断；保留原有明确 JS default host import 边界。不新增 Map 专属规则、诊断编号、运行时 callable wrapper 或 Dynamic 宽化，也不执行导入值来猜测类型。正常构造器仍由已有 constructor lowering 处理。

复用既有 check-known-assertion runner，通过 CLI 创建 source Map，定义上的 :tests 验证正常 get/unwrap。负例覆盖直接 alias 调用和局部保存后调用，在普通严格检查、JS、WASM 与 WASI 生成之前拒绝，并保留调用定义上下文。原有参数/返回、async、Option/Result、普通函数和 scalar WASM 回归保持通过；全部 416 项 core unit :tests 通过。

后续以合入 #1617 后的 main 为基线，完整验证与最新 HEAD review/CI 仍待完成；专项通过不冒充完整消费者或全量静态覆盖。
