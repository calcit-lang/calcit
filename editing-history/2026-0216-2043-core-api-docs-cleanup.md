# 2026-02-16 20:43 core API/docs cleanup

## Scope

- Cleaned outdated draft references and status notes.
- Aligned core internal API names around trait attachment (`*:impl-traits`).
- Fixed `wo-js-log` export/definition mismatch.
- Merged symbol-resolution note into drafts and removed duplicated doc file.

## Key updates

1. **drafts cleanup**
   - Added `drafts/README.md` as a state index (active/review-needed/archived).
   - Fixed stale links between `assert-types-plan.md` and `assert-types.md`.
   - Marked `drafts/last-session.md` as archived historical snapshot.

2. **core naming consistency**
   - Updated internal entries in `src/cirru/calcit-core.cirru`:
     - `&record:with-impls` -> `&record:impl-traits`
     - `&tuple:with-impls` -> `&tuple:impl-traits`
     - `&struct:with-impls` -> `&struct:impl-traits`
     - `&enum:with-impls` -> `&enum:impl-traits`
   - Updated `%::` doc hint to reference `&tuple:impl-traits`.

3. **debug macro fix**
   - Fixed `wo-js-log` entry to define `wo-js-log` (not `w-js-log`).

4. **symbol note consolidation**
   - Appended symbol-resolution appendix into `drafts/runtime-traits-plan.md`.
   - Removed duplicated `docs/symbol-spec.md` after migration.

## Validation notes

- Verified `wo-js-log` evaluates successfully.
- Verified `&struct:impl-traits` resolves as runtime proc.
- Confirmed old `&struct:with-impls` now reports unknown symbol (no compatibility layer, by design).

## Follow-up

- If needed, add explicit migration note/changelog entry for removed `*-with-impls` internal names.
