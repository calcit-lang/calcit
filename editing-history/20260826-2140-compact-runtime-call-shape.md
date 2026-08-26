# Cache compact native function call shape

- Continued issue #464 with a field-consumer and layout audit before changing
  the runtime representation. On Apple arm64, `Calcit` is 88 bytes,
  `CalcitFn` 176 bytes, `CalcitLocal` 48 bytes, and `CalcitScope` 24 bytes.
- Function values already share `Arc<CalcitFn>`, so ordinary calls do not clone
  generic, where-bound, return, or argument annotation graphs. Those fields are
  cold resident metadata rather than per-call allocation traffic. Local
  metadata is hot in the preprocessed tree, but shrinking `CalcitLocal` alone
  cannot shrink the 88-byte `Calcit` enum; a later material memory experiment
  needs a genuinely separate execution expression.
- Added a 6-byte `CalcitFnCallShape` that fills existing `CalcitFn` alignment
  padding, leaving `size_of::<CalcitFn>()` unchanged at 176 bytes. It caches the
  fixed parameter count, continuous trailing `Option` count, and combined
  marked/typed rest evidence after preprocessing.
- Native `run_fn` and `run_fn_owned` now consult the compact shape before
  optional-argument completion instead of scanning `CalcitFnArgs` and the full
  argument annotation list on every function call. Full schema metadata remains
  available to preprocessing, codegen, queries, effects analysis, and errors.
- Against main `86c47a93`, an alternating 30-pair native fold benchmark with
  200,000 callback calls changed from a 119.280 ms median to 117.647 ms; paired
  median change was -1.34%. A 15-pair 1,000,000-call run changed from 607.412 ms
  to 594.369 ms; paired median change was -2.09%.
- Latest Respo main (`27bb6304`) retained all 27 native tests; its 15-sample
  test-body median changed from 30.867 ms to 31.255 ms. Latest Recollect main
  (`e0903a6d`) retained all 9 native tests and stayed at a 9.520 ms median.
  These project-level differences are treated as noise rather than claimed
  speedups. Median peak RSS moved from 48,201,728 to 47,726,592 bytes for Respo
  and from 70,172,672 to 68,927,488 bytes for Recollect, but process-level RSS
  variance is too high to attribute that reduction to this zero-size cache.
- The release binary changed from 9,212,896 to 9,213,136 bytes (+240 bytes).
  No new heap allocation is introduced by the cached shape.
