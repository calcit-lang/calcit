# Native lowering for the core map macro

- Continued issue #436 after the merged native `let` fast path by lowering the
  strict core `{}` macro directly to `CalcitProc::NativeMap`.
- The fast path accepts only the macro's valid two-item list pairs, flattens
  them in source order, and leaves malformed pairs on the normal macro path so
  existing diagnostic text, locations, and metrics remain intact.
- Latest Respo main showed 96 `{}` expansions and about 2.91 ms spent in the
  general macro evaluator, making this compact, semantics-preserving lowering
  the next lower-risk target after `let`.
- Against the same-source #459 release binary on Apple arm64, latest Respo
  `--check-only` changed from a 240.10 ms median to 237.99 ms across 20
  alternating process pairs; paired median change was -1.43%. The macro metric
  evaluator total was about 35.0 ms before and 20.1 ms after, while `{}` itself
  moved from about 2.91 ms to zero. This remains compile/check performance,
  not an application-runtime claim.
- Full Calcit tests and `yarn check-all` passed. Latest Respo passed 27 tests
  and JS check-only; latest Recollect passed 9 native tests and emitted/ran its
  JS test entry successfully.
