# Quality baseline v2: explicit unsafe boundary budget

- `analyze quality` now collects explicit `unsafe-coerce` occurrences as the
  independent `unsafeCoerce` metric instead of folding them into Dynamic debt.
- Newly written native baseline files use schema version 2 and preserve the
  metric per definition, so an adapter moved to another definition cannot hide
  a regression.
- Native v1 and legacy flat eight-metric baselines remain readable and retain
  their original enforcement. Maintainers must review and regenerate a v2
  baseline before the new metric becomes a gate.
- Static quality only counts visible assertions. It does not claim that a
  Node/browser runtime contract test has executed; those remain an explicit Q3
  requirement.
