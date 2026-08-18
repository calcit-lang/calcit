# Review type-correctness follow-ups

- Kept named enum compatibility directional: a resolved enum `TypeRef` may satisfy a dynamic tuple parameter, but a dynamic tuple may not satisfy the named enum.
- Added namespace-qualified source identity to pre-runtime trait references so bootstrap fallback cannot confuse a user trait with a same-named core trait.
- Let unresolved core Option/Result constructors fall through to schema inference and taught method return inference to read impl methods from resolved enum `TypeRef` values.
- Added regressions for nominal trait fallback, source trait identity, and chained receiver-first Option/Result calls.
