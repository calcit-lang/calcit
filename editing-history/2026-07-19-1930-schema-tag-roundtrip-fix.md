# Schema kind tag round-trip regression fix (2026-07-19 19:30)

## Symptom

Running `cr tree insert-after`/`cr edit` on [calcit/test-traits.cirru](../calcit/test-traits.cirru)
(to add new `test-method-introspection` assertions) silently rewrote the `:schema`
field of *every other, unrelated* def in the same file from its specific kind tag
(`:enum`, `:trait`, `:impl`, `:struct`) down to `:schema :dynamic`. This affected
~15 defs (`Demo0`, `DemoBar`, `MyBar`, `MyBarImpl`, `MyFoo`, `MyFooImpl`, `Person0`,
etc.) that were never touched by the edit itself — any save of the file degraded
their schema fidelity as a side effect.

## Root cause

`:struct`/`:enum`/`:trait`/`:impl`/`:record` shorthand schema tags load into
`CalcitTypeAnnotation::Custom(Arc<Calcit>)` (see `CalcitTypeAnnotation::from_tag_name`
in [src/calcit/type_annotation.rs](../src/calcit/type_annotation.rs)), wrapping the
kind as a bare `Calcit::Tag`. On save, `schema_annotation_to_edn` (in
[src/snapshot.rs](../src/snapshot.rs)) converted the in-memory schema back to EDN via
`CalcitTypeAnnotation::builtin_tag_name()`, which only handles primitive scalar
types (`bool`, `number`, `string`, ...) and has no arm for `Custom`, `Record`,
`Struct`, `Enum`, or `Trait` — those all fell through the `_ => None` catch-all,
so `schema_annotation_to_edn` silently defaulted to `Edn::tag("dynamic")`.

Since `code_entry_edn_pairs` recomputes every entry's schema EDN from its in-memory
`CalcitTypeAnnotation` on *every* file save (not just for touched defs), any
`cr edit`/`cr tree` write to a file containing these shorthand-tagged schemas
degrades them all to `:dynamic`, regardless of which def was actually edited.

Note `builtin_tag_name()` is also used by `to_brief_string()` and other call
sites that intentionally want more specific output (e.g. `struct Person` instead
of a bare `:struct`) for those variants — so widening `builtin_tag_name()` itself
to cover them would have regressed those call sites. The fix instead lives
directly in `schema_annotation_to_edn`, which is the one place that needs the
short kind-tag representation for the snapshot file format.

## Fix

Added explicit match arms to `schema_annotation_to_edn` in [src/snapshot.rs](../src/snapshot.rs):
- `Custom(value)`: coerce the wrapped `Calcit::Tag` back into its `Edn::Tag` (falls
  back to `:dynamic` only if the wrapped value isn't a bare tag, which shouldn't
  happen via `from_tag_name`).
- `Record(_)` → `:record`, `Struct(..)` → `:struct`, `Enum(..)` → `:enum`,
  `Trait(_)` → `:trait`.

This preserves the user's original expressed intent (e.g. `:schema :impl`) instead
of silently downgrading it to `:dynamic`, which would otherwise turn off
schema-based type-checking coverage for those defs.

Recovered the previously-corrupted `calcit/test-traits.cirru` by reverting to
`git checkout` and reapplying the new `test-method-introspection` assertions with
the fixed binary — confirmed via `git diff` that this time only the intended
7 new lines changed, with zero incidental `:schema` churn.

## Regression test

`test_custom_kind_schema_tags_round_trip_instead_of_degrading_to_dynamic` in
[src/snapshot.rs](../src/snapshot.rs) asserts `schema_annotation_to_edn(CalcitTypeAnnotation::from_tag_name(kind))`
round-trips to `Edn::tag(kind)` for `struct`/`enum`/`trait`/`impl`/`record`.

## Validation

`cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (all pass),
`yarn check-all` (exit 0).
