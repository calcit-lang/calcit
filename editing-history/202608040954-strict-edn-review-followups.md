# Strict EDN review follow-ups

## Review findings handled

- Preprocessing now retains the derived `EdnDecoderGraph` as an internal `AnyRef` handle. Native evaluation and JS codegen reuse that graph instead of parsing and deriving the target type on every evaluation/codegen pass.
- `collect_compiled_deps` reads nominal paths from the retained graph, so same-namespace struct/enum declarations participate in JS top-level dependency ordering. The integration fixture deliberately names the decoded value `A-typed-person`, lexically before `Person`, and verifies codegen still initializes `Person` first.
- Native strict record decoding rejects duplicate EDN field names instead of collapsing them through `HashSet` comparison and selecting the first value.
- Recursive `TypeSlot` resolution has a dedicated in-progress guard and returns a type error instead of overflowing the preprocessing stack.
- JS runtime validates decoder-vs-nominal field alignment and unknown decoder node kinds explicitly.
- The decoder module and graph types are crate-private, matching the RFC boundary for Phase 1.
- Dynamic record fallback construction was simplified without changing its compatibility behavior.

## Review decisions

- The RFC/history date remains 2026-08-04. GitHub review timestamps were displayed in UTC on August 3, while the repository task timezone was Asia/Shanghai and local time was already August 4.
- Await-aware wrapping was applied to the generated strict decode call for consistency. With an empty argument prelude it is behaviorally equivalent, while the surrounding function/top-level code still owns async context detection.
