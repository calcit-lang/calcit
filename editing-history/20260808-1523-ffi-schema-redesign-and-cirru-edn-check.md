# FFI Schema Redesign + `cirru.edn` check-md Mode

- Retired the premature `CalcitTypeAnnotation::Host` variant and its temporary `defhost-type` macro. Host/FFI value types should not become a parallel type-annotation system.
- Rewrote the cross-backend host/FFI RFC around Calcit's existing conventions: function/value types stay on `CodeEntry.schema` using `:: 'Fn`/`:: 'Trait`/etc.; backend lowering metadata (JS host path, native symbol, WASM module/field) lives in a separate `CodeEntry.ffi` raw EDN field that never participates in ordinary type matching.
- Redesigned host capability declarations to reuse existing `deftrait`/`:where` machinery instead of inventing field/shape/member schema kinds; JS property get/set/call lowering is expressed as a small `:members` map on the trait's `:ffi` metadata.
- Added a `cirru.edn` fenced-block mode to `cr docs check-md`/`format-md` (`CirruCheckMode::Edn`, `run_edn_parse_only` in `src/bin/cli_handlers/docs.rs`) that validates a block parses as EDN data via `cirru_edn::parse`, for schema/`:ffi`-shaped snippets that are not runnable/parseable Calcit source. Documented it in `docs/run/cli-options.md`.
- Reclassified the RFC's `CodeEntry`/type-reference example blocks from `cirru.no-check` to `cirru.edn` (kept the one illustrative `deftrait DomInput` Calcit-code block as `cirru.no-check`) and verified all 13 blocks pass `cr docs check-md`.
