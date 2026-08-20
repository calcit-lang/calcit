# Agent guide cold-start validation

- Reworked the embedded Agent guide around Snapshot identity, Cirru AST boundaries, query-to-cursor workflows, structural editing, and layered validation.
- Preserved the high-frequency Cirru rules that fresh agents commonly misapply, while routing lower-frequency operations through live CLI documentation.
- Added practical cursor guidance for sibling insertion, clipboard state, inherited sidecars, focused previews, invalid navigation, and non-invertible unwrap operations.
- Corrected error recovery guidance so current stderr takes precedence over stale persisted runtime stacks.
- Validated the guide through isolated cold-start trials in a real Respo project without committing experimental changes.

Validation:

- `bash scripts/check-docs-md.sh` (52 files, 286 blocks)
- `yarn check-agent-interface` (12 scenarios)
- `cargo test cli_handlers::docs::tests::agents_docs_default_to_the_version_matched_embedded_guide`
- Isolated `cr --check-only` and `cr js` trials in `respo-calcit-workflow` copies

