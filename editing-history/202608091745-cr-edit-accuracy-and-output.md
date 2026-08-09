# Improve cr edit accuracy and output

Recent definition-test migrations exposed three CLI failure modes worth fixing:

- Running a source `calcit-core.cirru` through an older `cr` binary silently
  replaced its `calcit.core` namespace with the binary's embedded snapshot.
- Numeric tree paths could still point at a valid adjacent node, allowing a
  delete or insertion to succeed at the wrong location.
- Tree mutations repeated the same before/after fragments, command echoes
  included every inactive default, and the core suite printed hundreds of PASS
  rows during the normal repository gate.

Changes:

- Preserve namespaces supplied by the input snapshot while filling only missing
  core namespaces from the embedded snapshot.
- Add optional quoted-Cirru/JSON `--expect` guards to path-based replace,
  delete, and insert operations. A mismatch fails before the snapshot is saved.
- Include non-default snapshot paths and all active test filters in command
  echoes. Hide inactive/default options unless `--verbose` is requested.
- Remove duplicated tree insertion/deletion previews, honor `--depth`, and show
  the actually modified container after child insertion. Run
  `yarn try-core-tests` with `--summary-only`; summary mode also suppresses
  program output from intentionally failing assertion tests.

Focused validation:

- Unit tests for exact/mismatched node guards and source-core precedence.
- A guarded deletion with the wrong expected node exited unsuccessfully and
  preserved the temporary snapshot SHA-256.
- A temporary source core containing one extra test ran 211 tests while the
  binary embedded 210, proving the source namespace won.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`,
  `yarn compile`, `yarn check-agent-interface`, and `yarn check-all` passed.
