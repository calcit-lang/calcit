# Document Deprecated API Cleanup

在 `docs/CalcitAgent.md` 增加简短的 deprecated API 清理流程：用
`cr analyze deprecated` 定位调用，以 summary-only JSON 作为 migration gate，并在
目标范围 calls 清零前保留兼容 API 与 `:deprecated` tag。

验证：`cr docs check-md docs/CalcitAgent.md --entry calcit/test.cirru`。