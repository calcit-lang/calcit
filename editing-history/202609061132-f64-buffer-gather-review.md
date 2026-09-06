# 收口 F64Buffer gather review / Close F64Buffer gather review findings

## 中文

- CodeRabbit 指出 gather 文档把实际使用的两个 intrinsic 误写成三个；文档现明确列出 `F64BufferGet` 与 `F64ToI64Index`，不再暗示使用 `F64BufferLen`。
- zero-remaining 用例此前只检查 Calx 返回 `7`。现在同参数也通过 native Calcit 执行，并显式断言 native/Calx 相等，使文档中的 differential coverage 与测试一致。
- 新增 fixture helper 和综合回归测试补充简短职责说明，满足 touched-function docstring 检查而不增加公共 API 文档噪声。
- CI 暴露的 `unsafe-coerce` 测试隔离修复由 #886 单独记录并继续由本 PR 携带，避免当前 gather PR 在独立修复合并前重新暴露竞态。

## English

- CodeRabbit identified that the gather documentation described three intrinsics although the fixture uses two. It now names `F64BufferGet` and `F64ToI64Index` explicitly and no longer implies `F64BufferLen` usage.
- The zero-remaining case previously checked only the Calx result of `7`. It now executes the same arguments through native Calcit and asserts native/Calx equality, aligning the differential-coverage claim with the test.
- Brief responsibility comments now cover the new fixture helper and comprehensive regression test, satisfying the touched-function docstring check without adding public-API documentation noise.
- Issue #886 separately records the `unsafe-coerce` test-isolation fix exposed by CI while this PR continues to carry it, avoiding renewed flakiness before a separate dependency could merge.
