# Repository maintenance review

- Verified TypeScript compilation, Rust clippy, and the full Rust test suite;
  no safe dead-code removal was identified in the current codebase.
- Reclassified the README module configuration example as `cirru.no-check`:
  it is snapshot data, not runnable Calcit. This restores Markdown example
  validation under the current required-Struct-field analysis.
- Replaced the obsolete `cr config version` transaction smoke operation with
  `edit doc`; project versions now belong in `deps.cirru` and are managed by
  `caps version`, while the test still exercises a two-operation dry run.
- Consolidated 47 overlapping cursor, module-cache, and release checkpoints
  into `ARCHIVE.md`. The archive preserves durable behavior and directs readers
  to the current operational documents; exact notes remain available through
  Git history.
- Updated history retention guidance so a completed, high-churn topic is
  consolidated even within the normal current-development window.

Validation: `yarn compile`, `cargo clippy --all-targets -- -D warnings`,
`cargo test -q`, `yarn check-agent-interface`,
`cr docs check-md README.md --entry calcit/test.cirru`, and
`./scripts/check-docs-md.sh calcit/test.cirru` for the Markdown under `docs/`.
