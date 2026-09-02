# Prefer Cirru EDN quality baselines

## Context

The bundled core quality baseline was stored as JSON even though it is an
internal Calcit-owned configuration artifact. JSON remains useful for external
tooling and machine report envelopes, but it should not be the default
persistence format for repository-owned Calcit data.

## Changes

- Make quality baseline writing format-sensitive: every non-`.json` target is
  written as Cirru EDN, while an explicit `.json` target keeps JSON output.
- Read native and legacy flat Cirru EDN baselines, while preserving existing
  JSON baseline compatibility.
- Migrate the bundled baseline to `config/calcit-core-quality.cirru` and update
  package scripts, CI, release workflow, docs, and the adoption RFC.
- Mark `config/*-quality.cirru` as `text linguist-generated=true` so generated
  baseline lines do not affect GitHub language statistics while textual diffs
  remain available for review.
- Add round-trip tests for native Cirru EDN, explicit JSON output, and legacy
  flat Cirru EDN input.

## Validation

- Cirru EDN write/read gate passes with the existing 297 unresolved budget.
- Two consecutive baseline generations produce the same SHA-256 hash.
- `git check-attr` reports `text: set`, `linguist-generated: true`, and leaves
  textual diff enabled.
- Full repository gates run after the migration.
