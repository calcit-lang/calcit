# 解析方法的跨后端契约

关联 #1456 / #1458 / #1600。已有 JSON/Cirru/Cirru EDN/List `result-method-contract` 测试使用普通 String 方法，原 native/JS decoder 检查却只提取闭合 decoder 的测试。扩大同一检查脚本的 AST 来源，并为 `.parse-float` 补成功、失败与 Result 类型断言，不以 Node host 断言或 primitive 替代语言方法。

四个 parser 函数的声明纳入原生 EDN 候选基线；开放 JSON/Cirru EDN/List payload 不伪造为具体类型，也不纳入要求 proven 的方法清单。契约工具继续比较当前及 Git 历史；新增负例保护已知的 String 错误类型，原有行不变。

首次回归发现 `)` 在 native 为 Result err、生成 JS 却为 ok。缺陷属于 `@cirru/parser.ts@0.0.9` 的 lexer，见 #1600 与 Cirru/parser.ts#42 / PR #43。保留失败样例和现有错误模型，在 owner 修复并正式发布版本后升级依赖；不在 Calcit 增加第二套 parser，不更换测试预期，也不把未发布 checkout 当作发布兼容证据。

本批不扩张 WASM parser 能力。闭合 Cirru EDN WASM decoder 继续由其原有支持矩阵与实际执行测试证明。
