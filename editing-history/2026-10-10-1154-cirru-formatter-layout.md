# Cirru formatter 布局参数对齐

`format-cirru` 的 native 实现一直给 writer 固定传入 false，而 JS 转发显式 Bool。已发布 alpha.26 配对在树 `([] ([] |a ([] |b |c) ([] |d |e)))` 上可复现：true 应生成单行 `a (b c) (d e)`，native 却输出两行；省略参数与 false 在两端都使用展开布局。

保留默认 false，仅修正显式 true，并在 native/JS 的宿主入口拒绝非 Bool。core 声明补齐 Bool，继续由既有 runtime arity 表达可省略；没有新类型关系、语法、命令或动态逃生口。失败说明留在 proc/doc，契约基线由已有 query 导出，不另外抄写失败文字。

扩展原 `formats-nested-lines` 附带测试，保留原断言，增加 direct、局部别名、一元 callback 与两种布局断言。修复前该测试 native 在 true 处失败、生成 JS 通过；修复后用同一 AST 验证。非 Bool 负例扩展既有 parse-boundary 与 Rust 宿主测试；WASM/WASI 仍 unsupported，原排除项不变。

本次是未发布候选修复，不移动 alpha.26 tag；与 EDN formatter 默认 true 保持区别。完整 CI/review 与精确合并 main 验证完成后才进入发布。对应 #1900、#1456。
