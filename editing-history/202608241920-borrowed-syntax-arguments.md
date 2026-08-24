# Borrowed syntax arguments

## Context

After ordinary executable calls moved to contiguous `CalcitList::Vector` storage, syntax dispatch still built a persistent tail with `skip(1)` on every evaluation. Converting syntax roots to vectors before changing that API regressed hot loops because each syntax call reconstructed a list.

## Changes

- Added `CalcitListView`, a borrowed read-only range over either vector or persistent list storage.
- Routed `call_expr`, argument evaluation, syntax dispatch, and ref syntax handlers through borrowed views.
- Kept allocation explicit with `to_vec` only where owned stack/error data or function bodies require it.
- Normalized all preprocessed executable call roots, including syntax calls, to contiguous storage while preserving quoted list values.
- Added view coverage for both storage backends, nested skips, iteration, indexing, and invalid bounds; updated preprocessing representation coverage.

## Performance

The release benchmark runs a two-argument tail-recursive loop for 1,000,000 iterations and returns `500000500000`.

- `main` (`1ef20077`) internal median: 7,062.173 ms; user CPU median: 7.06 s.
- This change internal median: 5,920.961 ms; user CPU median: 6.00 s.
- Improvement: 16.2% internal runtime and 15.0% user CPU.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -q`
- `yarn compile`
- `yarn check-agent-interface`
- `yarn check-all`
- Latest Recollect `a694d4c`: native tests 9/9, test-entry JS generation and Node runtime passed. Its default entry retains the same four pre-existing dependency type warnings.
- Latest Respo `ead78c3`: tests 25/25, JS generation and Vite production build passed. Browser Todo interaction added one task and produced zero console errors.

The downstream Dynamic coverage remains unchanged at 204/405 (50.4%) for Recollect and 284/1034 (27.5%) for Respo.
