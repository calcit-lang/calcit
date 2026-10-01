# 数字 refinement 的持续回归与后端证据

## 决策

承接 #1456/#1458 与 #1603 的精确声明基线，把十个既有 `number->int*/uint*/float*` definition `:tests` 纳入已有 `check-numeric-predicate.mjs`，不另建检查器、CLI 或统计规则。native 与生成 JS 执行完整的原始 AST；断言继续由 Calcit 源码拥有，Node 只负责临时 Snapshot、宿主执行及非空选择校验。

既有 Int8 测试还验证 throwing `parse-cirru-edn-as` 与 refinement 容器的关系。把全部测试直接交给 WASM 的试跑明确被该未支持入口拒绝，不能通过删去断言来宣称后端对齐。WASM 与真实 WASI 0.3 Component 保留原有四个整数谓词 AST；日志分别说明覆盖范围。

## 边界

不修改任何语言实现、schema、期望值或测试源，不放宽 Dynamic，不删除别名，不提供 baseline 豁免。文档增加关键 API 的测试证据索引，区分静态 `proven`、codegen 和实际运行；它不是完整 core 支持矩阵或新发布消费者兼容证明。修正 WASM 文档中“CI 只运行 test-wasm.sh”的过时说明。

## 验证

复跑修改后的已有脚本，保留十个转换与四个谓词测试，native/生成 JS 通过；core WASM 与 Wasmtime 的默认 Component 通过原有四个谓词测试。契约工具、文档检查与 PR CI 继续验证；本批不因此关闭 milestone 的其他冻结、迁移和发布门槛。
