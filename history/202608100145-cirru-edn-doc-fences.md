# Standard Cirru EDN documentation fences

## Background

Executable documentation fences use the standard `cirru.edn` marker. The
non-standard `cirru-edn` spelling bypassed EDN validation and hid malformed
configuration and schema fragments.

## Change

- Replaced all newly introduced `cirru-edn` fences with `cirru.edn`.
- Wrapped standalone configuration/schema fields in top-level EDN maps.
- Quoted symbolic trait references inside EDN schema values.

## Verification

- `bash scripts/check-docs-md.sh`: 54 files and 290 blocks passed.
