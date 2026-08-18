# Documentation type syntax audit

- Updated the trait guide and quick reference so schema and data-declaration type positions use quoted symbols (`'String`, `'Number`, `'Fn`, and so on).
- Kept ordinary tag data unchanged, including enum variants, record keys, and schema-map keys such as `:return`.
- Ran `docs format-md` and `docs check-md` with `calcit/test.cirru` for both updated guides.
- The wider audit still finds legacy type-tag examples in feature, run, and migration pages. Upgrade/compatibility explanations deliberately retain old spellings; the remaining tutorial/reference pages should be migrated in a dedicated documentation sweep with code-block validation.
