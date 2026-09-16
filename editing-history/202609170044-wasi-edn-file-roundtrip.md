# WASI typed Cirru EDN file roundtrip / WASI 有类型 Cirru EDN 文件往返

## 中文

- 在 Calcit fixture 中组合已有的 typed Cirru EDN parser、formatter 与预开放文件 API。
- 补齐 WASM 的 `&number:fits?`，让业务代码可以在恢复 numeric refinement 前执行同源边界检查。
- 增加 `Map<String, Int32>` 的业务变换测试；算术结果显式检查并恢复 refinement，同时保留越界错误。
- 回归脚本验证有效输入的读、改、写与重新解析，也验证错误类型输入以状态码 `1` 退出且不产生输出。
- 更新 WASM 验证文档，明确首个端到端数据文件流程与当前边界。

## English

- Compose the existing typed Cirru EDN parser, formatter, and preopened-file API in a Calcit fixture.
- Implement WASM `&number:fits?` so business code can perform the canonical bounds check before restoring a numeric refinement.
- Add a business transformation test for `Map<String, Int32>`; check and restore the arithmetic refinement explicitly while preserving overflow errors.
- Verify read-transform-write-reparse for valid input and exit status `1` without output for invalid typed input.
- Document the first end-to-end WASI data-file workflow and its current boundary.
