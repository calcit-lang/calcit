# Component v4 文档一致性 / Component v4 documentation consistency

## 中文

- 统一文档中的当前 Component Interface IR 版本为 v4。
- 明确 `readable-byte-stream` contract 已收敛，但 core consumer adapter 完成前仍 fail closed。
- 升级说明区分 native IR v3 与 Component IR v4，避免继续导出旧 Component contract。

## English

- Use Component Interface IR v4 consistently in current documentation.
- Clarify that the `readable-byte-stream` contract is defined while core generation remains fail closed until the bounded consumer adapter lands.
- Distinguish native IR v3 from Component IR v4 in the upgrade workflow.
