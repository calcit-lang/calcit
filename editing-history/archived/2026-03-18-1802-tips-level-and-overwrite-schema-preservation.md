## Summary

This commit finalizes two CLI workflow improvements:

1. **Tips policy refactor**
   - Added `--tips-level` support (`minimal|full|none`) as a unified switch.
   - Kept `--tips` as a shortcut for full tips output.
   - Updated handlers so tips rendering is centralized and priority-aware.
   - Updated docs to align with the new default behavior (minimal, high-priority-first hints).

2. **`cr edit def --overwrite` metadata safety**
   - Fixed overwrite behavior to preserve existing definition metadata (`doc`, `examples`, `schema`).
   - Overwrite now updates only the `code` field when the definition already exists.
   - This avoids accidental schema loss during whole-definition rewrites.

## Files touched (high level)

- CLI args and command wiring for tips level handling.
- Query/tree/tips handler integration and output behavior updates.
- `edit` handler overwrite logic for metadata-preserving updates.
- Agent docs and advanced workflow docs reflecting the new tips model.

## Validation notes

- Rust formatting/lint/tests were run during implementation iterations.
- Manual end-to-end verification confirmed schema is retained after `--overwrite`.
