# Upgrade ternary-tree to 0.0.21

- Upgraded `im_ternary_tree` from 0.0.20 to 0.0.21. The upstream release
  removes redundant cloning in owned vector construction and finger-tree
  updates, and reports substantially faster full iteration and search
  microbenchmarks.
- Compared optimized Calcit binaries built from the same source snapshot, with
  only the ternary-tree version changed. Runs alternated old/new order on Apple
  arm64 to reduce thermal and background-load drift.
- Over 30 paired process runs, `--check-only calcit/test.cirru` improved by
  about 0.83% at the median and the native literal-path benchmark improved by
  about 0.39%. Over 20 paired macro-metric runs, post-preprocessing improved by
  about 1.22%, while macro evaluation remained effectively unchanged.
- Treat the small aggregate gains as evidence of no regression plus a modest
  end-to-end benefit. Calcit parsing, type analysis, evaluation, and process
  startup dominate these workloads, so the upstream collection microbenchmark
  gains are not expected to transfer proportionally to whole-program timings.
- Revalidated Rust formatting, Clippy with warnings denied, the full Rust test
  suite, self-hosted compilation, Agent CLI protocol checks, native/JS/IR/WASM
  integration tests, and collection-path performance smoke tests.
