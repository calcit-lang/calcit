# match syntax with exhaustiveness checking

## Summary

Added native `match` syntax for enum pattern matching with compile-time exhaustiveness detection,
as a safer alternative to `tag-match` macro. See `drafts/match-syntax-rfc.md` for full design.

## Files modified

- `src/calcit/syntax_name.rs` — `CalcitSyntax::Match`
- `src/builtins/syntax.rs` — `syntax_match()` runtime
- `src/builtins.rs` — dispatch entry
- `src/runner/preprocess/mod.rs` — `preprocess_match()` with exhaustiveness
- `src/codegen/emit_js.rs` — `gen_match_code()` JS output
- `calcit/test-enum.cirru` — test cases

## Key takeaway

Cirru indentation creates pair-based `(pattern body)` children — same format as `tag-match`.
Flat alternating format doesn't survive Cirru serialization round-trips.
