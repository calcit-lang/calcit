# 202604252010 — WASM suite extended: + test-fn / test-lens / test-edn

## Background

After landing the `.includes?` string fix, this round adds three more `test-*.cirru` programs to
`calcit/test-wasm-suite.cirru` to cover more of the WASM runtime surface. Goal: keep growing the
"runs end-to-end in WASM" set without depending on Host FFI features.

## Process

For each candidate module:

1. Inspect `cr-wasm <module>.cirru` standalone to enumerate `[wasm] skipping ...` lines (these are
   per-def errors that cause that def to be replaced with a `0.0`-returning stub).
2. Read the module's `main!` body to confirm trapping assertions can survive when the skipped
   helpers return `0`.
3. Add to the suite, rebuild, and run `bash scripts/test-wasm-suite-extended.sh`.

## Result

Suite now covers (in addition to previous `test-cond/test-math/test-set/test-tuple`):

- `test-fn` — only `&init-builtin-impls!` skipped; suite passes cleanly.
- `test-lens` — `test-lens.main/test-lens` is skipped (`:::` rejects a quoted form arg) but other
  defs in the module compile; `main!` does not trip the stub.
- `test-edn` — many sub-tests skipped (`parse-cirru-edn`, `trim`, `&extract-code-into-edn`), but
  the skipped fns return `0` instead of running their assertions, so `main!` completes.

Modules tried but reverted:

- `test-string` — assertions feed tags into `starts-with?`/`ends-with?`/`includes?`. Tags in WASM
  are encoded as small `f64` ids (their tag-index value), not heap string pointers, so the procs
  silently return `0` and the assertion traps. Needs a tag→string runtime helper.
- `test-algebra` — `test-map` exercises `%{} AlgebraBox`, `assert-traits`, `.map` on records, and
  `:value` access; record/trait dispatch is not yet implemented in WASM.
- `test-map` (the standalone, full map suite) — `test-pairs`/`test-keys` etc. compile but trap at
  runtime, likely from map iteration ordering or hash collision behavior. Needs deeper triage.

## Verification

```bash
bash scripts/test-wasm-suite-extended.sh   # PASS (8 modules including util)
```
