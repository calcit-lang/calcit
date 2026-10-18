# 主线同步 JavaScript epoch 毫秒时钟

维护修复 #2161 补齐已有 `unix-time-ms` 的 JS runtime 导出；主线存在相同缺口，不能让后续版本重新出现“strict/codegen 通过、浏览器运行失败”。本次仅同步同一实现、附带测试、宿主测试与文档，不改变维护分支的范围，也不把主线后续语言变化回移。

JS 读取一次 `Date.now()`，返回 epoch 毫秒；合法零值保留，负值按 native 合同失败，宿主异常原样传播。单调时钟、公开签名及原测试保持不变。用户语义放在 definition `:tests`，具体 JS 宿主边界继续复用已有时钟脚本，不新增 runner。core WASM 与 WASI 0.3 command 的现有不支持边界不变，仅登记新增测试对应的排除。

Snapshot 先 query，再使用同一结构化 transaction dry-run 与 definition-scoped revision 守卫，只修改该定义文档与测试。主线需独立完成生成 core 调用、同源 native/JS 测试及最新提交 review/CI；维护候选或旧发布版的结果不冒称精确主线验收。新正式配对与真实消费者浏览器验收仍由 #2159/#2080 追踪。
