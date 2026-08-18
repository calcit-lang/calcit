# Markdown Cirru formatter

## What changed

- Added `cr docs format-md <file>` to canonicalize fenced `cirru`, `cirru.no-run`, `cirru.no-check`, and `cirru.cli` blocks in Markdown.
- Added `--check` for CI: it reports non-canonical blocks without writing the Markdown file.
- Formatting preserves non-Cirru Markdown and fence labels, parses code directly as multiple Cirru AST roots, and writes using the existing atomic replacement helper.
- Added CLI help, command-echo support, unit coverage for formatting, parse errors, idempotence, and the non-writing check mode.
- Documented usage in CLI options and library quality guidance.

## Verification

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-all`
- `cr docs format-md --check` and `cr docs check-md` on affected documentation.
