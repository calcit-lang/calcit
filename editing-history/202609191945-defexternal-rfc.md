# defexternal 简写声明 RFC（#1223 设计先行）

- 新增 `RFCs/09-19-external-object-declaration-shorthand-rfc.md`：提案 `defexternal` 简写声明
  external-object trait，一处表达字段/方法、`:names`、`:writable`、`:target`，lowering 为与手写
  `deftrait` + `CodeEntry :ffi` 逐字段等价的产物。
- 关键决策：MVP 采用“载入期规范化”，不保留 `defexternal` 源码头，从而 preprocess/type/codegen/query
  零改动；不新增类型变体、不改变普通 trait 与 `impl-traits`/泛型语义；字段默认只读。
- 覆盖语法提案、lowering 与 Snapshot 表示、类型与 query 行为、兼容性、稳定错误、备选方案、
  开放问题、验收与实施阶段（P1 规范化 / P2 `calcit edit` 入口 / P3 可选保留源码头）。
- 同步更新 `RFCs/README.md` 索引表。
- 后续按 RFC 的 P1 实现，再进入 #1224。
