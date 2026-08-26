# Native lowering for the core let macro

- Added a compiler-native lowering for the strict, capability-free
  `calcit.core/let` macro. It builds the same nested `&let` tree as the macro's
  recursive quasiquote implementation, avoiding general evaluator execution
  for valid binding-pair lists.
- Invalid outer binding shapes deliberately remain on the ordinary macro path,
  preserving the established macro diagnostics and error behavior.
- After review, native lowering was narrowed to the exact `&let` binding
  contract: `()` or a two-item list headed by a symbol. Singleton, oversized,
  and non-symbol-headed lists therefore retain the ordinary macro path.
- Macro metrics now explicitly classify this route as `native-fast-path`, not
  as a general-evaluator fallback or a cache candidate.
- Latest Respo main release-binary metrics changed from 1,418 expansions / about
  29.04 ms evaluator time to 1,255 expansions / about 22.55 ms evaluator time.
  The `let` evaluator time became zero while its required post-preprocessing
  still runs.
- On Apple arm64, 20 alternating paired Respo `--check-only` processes against
  a same-source 0.13.46 baseline improved the median from 246.12 ms to
  237.40 ms; paired median change was about -2.96%. This is a process-level
  check improvement, not an application-runtime claim.
