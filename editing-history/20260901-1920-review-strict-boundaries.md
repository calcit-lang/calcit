# Strict boundary review follow-up / 严格边界审查跟进

- Strict eval/exec already performs its mandatory preprocessing pass before
  evaluation. The strict quality gate must reuse that result instead of
  preprocessing the same entries twice.
- Raw Struct index diagnostics may receive malformed or legacy IR without a
  Tag field operand. In that case the remediation must show the generic valid
  `(:field value)` form, never synthesize `(<unknown field> value)`.
- These review fixes preserve strict-mode behavior while avoiding duplicated
  work and keeping diagnostics directly actionable.
