# Preserve builtin semantic tags / 保留 builtin 语义标签

- PR #919 review correctly identified that the new source-less builtin JSON branch discarded SpecialBuiltinQueryMeta.semantic_tags. Serialize the authoritative tags and assert that to-js-data retains js-ffi classification. Legacy output remains unchanged.
- 修复有效 bot comment，补充端到端协议测试；不改普通定义与消费者 API。
- Additional release-profile evidence before this metadata-only follow-up: DomElementHost complete JSON 2597 bytes vs 0.14.2 legacy 3282 bytes; warm queries roughly 11–14 ms on both. Candidate binary 9,813,248 bytes vs installed 0.14.2 9,834,032 bytes. First invocation includes OS loader effects and is not used for warm comparison.
- js-ffi catalog parity: all 149 records match the released baseline in every field except the intentionally updated inspect command.
