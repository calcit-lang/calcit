# Documentation validation for required Struct fields

## Background

The PR's local `yarn check-all` suite passed, but GitHub's Test workflow also
runs `scripts/check-docs-md.sh`. Several executable documentation snippets
still relied on the former context-sensitive field syntax.

## Documentation migration

- Map reads that branch on absence retain their `Option` value for `tag-match`
  instead of unwrapping it first.
- Generic enum payloads, local trait implementations, generic Struct bodies,
  and anonymous Struct examples use explicit `&struct:get` when their receiver
  has no statically resolvable named Struct declaration.
- Snapshot/schema fragments now use `cirru-edn` fences so `check-md` validates
  them as data instead of evaluating tags such as `:entries` or `:where` as
  required field access.

## Verification

- `bash scripts/check-docs-md.sh`: 54 files, 286 blocks, all passed.
