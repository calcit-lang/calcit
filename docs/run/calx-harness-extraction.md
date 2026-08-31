---
title: "Completed Calx harness extraction"
summary: "Record the completed benchmark-product cutover and the compiler/runtime boundary retained in Calcit core."
scope: "core"
kind: "maintainer-guide"
category: "run"
aliases:
  - "Calx harness extraction"
  - "Calx benchmark ownership"
entry_for:
  - "calcit-calx-bench"
---

# Completed Calx harness extraction / Calx 基准工具拆分完成

## 中文

拆分由 [#558](https://github.com/calcit-lang/calcit/issues/558) 与
[#559](https://github.com/calcit-lang/calcit/issues/559) 追踪。standalone harness 已固定合并后的
Calcit revision，改用 `calcit-calx-benchmark-session/1` adapter，并在 Ubuntu/macOS CI 与
clean-state 完整 scalar matrix 中保持 correctness 全真和可追溯 provenance。

因此 core 不再发布 `calcit-calx-bench`，也不保存 process/settings orchestration、机器相关报告
或 benchmark-product contract。core 继续拥有 adapter、lowering/cache/runtime correctness 和
权威 scalar fixture；外部 harness 通过精确 revision pin 消费这些能力。

## English

Issues [#558](https://github.com/calcit-lang/calcit/issues/558) and
[#559](https://github.com/calcit-lang/calcit/issues/559) track the cutover. The standalone harness pins the
merged Calcit revision, consumes the `calcit-calx-benchmark-session/1` adapter, and preserves correctness
and provenance across Ubuntu/macOS CI and the complete clean-state scalar matrix.

Core therefore no longer publishes `calcit-calx-bench` or owns process/settings orchestration,
machine-specific reports, or benchmark-product contracts. It continues owning the adapter,
lowering/cache/runtime correctness, and authoritative scalar fixture; the external harness consumes that
surface through an exact revision pin.
