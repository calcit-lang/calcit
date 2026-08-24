# Indexed enum match review fixes

- Kept indexed JS match control flow inside an IIFE with real branch returns, so non-tail and assignment contexts cannot leak into wildcard or no-match handling after a successful case.
- Included the matched value expression in `js-await` detection while continuing to exclude nested function scopes.
- Added native runtime coverage for indexed branch selection and arity-mismatch wildcard fallback.
- Added JS codegen coverage for non-return labels and awaited matched values.
- Re-ran formatting, Clippy, Rust tests, TypeScript compilation, Agent interface checks, and the native/JS/WASM integration suite.
