# Edit History - Type Introspection Consistency Fixes

- **Objective**: Close gaps found while researching "how to discover a type's fields/methods in Calcit" (methods via traits/impls, fields via records, Display formatting). Documented findings and a prioritized fix plan in `RFCs/07-19-type-introspection-consistency-rfc.md`, then implemented items 1-3 (item 4 deferred as optional/out of scope for this pass).

- **Fix 1 — `Enum` `Display` now includes variants** (`src/calcit.rs`):
  - Previously `Calcit::Enum` printed only `(%enum :Name)`, discarding the already-tracked `variants()` (tag + payload types). Now prints `(%enum :Name (:variant1 type ...) (:variant2 ...) ...)`, matching the richness of `Struct`'s Display.
  - Test: `enum_display_includes_variants` in `src/calcit.rs`'s test module.

- **Fix 2 — `&methods-of`/`&inspect-methods` accept bare `Struct`/`Enum`/`Trait` values** (not just instances):
  - `src/builtins/meta.rs`: `collect_impl_records_for_value` gained `Calcit::Struct`/`Calcit::Enum` arms (read their own `impls` field directly). `iter_impls_in_precedence_order` now treats `Struct`/`Enum` the same as `Tuple`/`Record` (last-attached impl wins, i.e. reversed iteration) since `&struct:impl-traits`/`&enum:impl-traits` also `.extend()` impls onto the end.
  - `Calcit::Trait` handled as a special case (new helper `trait_dot_method_names`) since traits declare methods directly, not via an `impls` list — no `impls` field exists on `CalcitTrait`.
  - JS target parity: `ts-src/calcit.procs.mts`'s `lookup_impls` gained `CalcitStruct`/`CalcitEnum` branches; the `reverse`-order flag (previously only `CalcitRecord || CalcitTuple`) now also includes `CalcitStruct || CalcitEnum`; `_$n_methods_of`/`_$n_inspect_methods` special-case `CalcitTrait` the same way as the Rust side.
  - Test: `calcit/test-traits.cirru`'s `test-method-introspection` — new `let` block calling `&methods-of` on `(impl-traits Person0 MyFooImpl)` (bare struct), `DemoBar` (bare enum), `MyFoo` (bare trait), asserting the expected method tags show up. Edited via `cr tree insert-after` (module entry must be the module's own file, e.g. `cr calcit/test-traits.cirru ...`, NOT the aggregated `calcit/test.cirru` — `cr tree`/`cr edit` couldn't resolve the namespace when invoked through the aggregator, while `cr query` could).

- **Fix 3 — `to-pairs`/`keys` type signature no longer false-warns on records**:
  - `src/calcit/type_annotation.rs`: `matches_with_bindings`'s `(TypeRef, Record)` arm previously only matched if the `TypeRef` name equaled the record's own struct name (e.g. `"map"` vs `"Person"` → false). Added: when the `TypeRef` name is the generic `"map"` placeholder, match any record structurally (records are field-name → value, i.e. map-like), in addition to the existing exact-name match.
  - Test: `generic_map_type_ref_accepts_records_structurally` in `src/calcit/type_annotation.rs`'s test module — verifies `TypeRef("map")` matches `Record(Person)` both directions, and confirms unrelated `TypeRef` names still don't match structurally (no over-broadening).
  - Note: empirically, `cr --check-only` on `test-record.cirru` didn't show a warning for `keys p2` even before this fix (likely because that call site's static type inference doesn't currently narrow `p2` to `Record`), so this is a defensive/forward-looking fix for when static inference improves or explicit type annotations are used, not a fix for an observed regression today.

- **Deferred (RFC item 4, optional)**: `&struct:fields`/`&enum:variants` new introspection procs — would require registering a new `CalcitProc` end-to-end (proc_name.rs signature + string mapping, builtins.rs dispatch, records.rs/meta.rs impl, JS/IR/WASM codegen parity). Judged lower priority/cost-to-benefit than items 1-3 which fixed real capability gaps; left as a follow-up.

- **Tooling note**: `cr tree`/`cr edit` subcommands need the entry file to be the *specific module's own `.cirru` file* (e.g. `calcit/test-traits.cirru`) to resolve `<ns>/<def>` targets — passing the aggregator entry (`calcit/test.cirru`, which just lists the module in `:modules`) causes `Error: "Namespace '...' not found"` for `tree`/`edit` even though `cr query defs/def` works fine through the aggregator. `--file` snippet content must be `quote`-prefixed (e.g. `quote (let (...) ...)`) even though the repo's `Agents.md` `--stdin` removal note doesn't mention this requirement explicitly.

- **Validation**: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` (239 lib tests incl. 2 new), `cargo run --bin cr -- calcit/test.cirru`, `yarn check-all` (compile + try-rs + try-js + try-ir + try-wasm) — all clean/passing.
