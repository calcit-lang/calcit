# trait 特化后的 callback payload 证明

## 背景

`calcit.core/map` 的公开 schema 使用 `C: Mappable` 表达多种容器，因此 `C` 与 callback
payload 的具体关系不在 facade 签名中。虽然预处理已经会根据 receiver 生成
receiver-specialized contract，但严格泛型门禁此前只验证公开 schema，导致
`List<Dynamic>` 可以绕过证明并进入只接受 `Number` 的 callback。

## 调整

- 在已有 checked call contract 生成后，用统一 `TypeProof` 验证处理完成的实参与特化契约。
- 只把 `Dynamic` 向具体 payload 的隐式收窄提升为现有 `E_ERASED_GENERIC_RELATION`；安全的
  `Dynamic` 保存、传递和返回保持合法。
- 诊断报告 callee、参数位置、实际类型、特化后的期望类型以及显式 decode/narrow 方向。
- 不增加 callback 专属 analyzer、计数器、provenance 分类或新的 warning/error code。

## 测试

- `calcit.core/map` 与 `calcit.core/option:map` 的 definition `:tests` 增加开放 callback
  正例，直接从 Calcit 用户视角验证 `List<Dynamic>` 与 `Option<Dynamic>` 可以被保持开放地映射。
- `calcit.core/map` 另以 `FsPath` 列表回归匿名 callback 的 Struct 字段 receiver 推断，确认
  本问题没有掩盖第二个独立根因。
- Rust 集成测试只保留严格失败边界：公共 `map` 必须拒绝 Number-only callback，并校验
  稳定错误码和机器可用的诊断字段。
