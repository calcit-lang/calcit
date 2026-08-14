# 2026-08-14 16:40 — architecture scaffold review fixes

## Summary

- Processed PR #351 review comments for scaffold reconciliation, metadata preservation, machine-result revisions/diffs, work-item identity, and writer concurrency.
- Made explicit function code, tags, examples, and data code survive scaffold apply; generated `:scaffold` only for TODO function stubs.
- Added empty-root validation, external target compatibility validation, per-definition diffs, proposed/applied revisions, and post-scaffold work-item revisions.
- Added a destination writer lock around staged snapshot replacement and strengthened the named-cursor round-trip fixture with distinct states.
- Enforced one `todo!` argument contract across preprocessing, JavaScript, WASM, and native boundaries; diagnostics now identify the containing definition and invalid messages use the argument location.
- Expanded the machine-protocol RFC with canonical JSON tagged projections and stdio framing/negotiation fixtures, and aligned the cursor default example with `:default`.

## Validation

- `cargo fmt --all`
- `cargo clippy -- -D warnings`
- `cargo test`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`
- `cr docs check-md` for all modified RFC and static-analysis documents

