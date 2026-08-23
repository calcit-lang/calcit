# Fix Option and Result binding macro method order

- Corrected `option:let` and `result:let` expansion to call `.and-then` in Calcit's method-first form.
- Added regression tests where a binding is an evaluated expression rather than a literal container.
- Corrected the RFC expansion example and linked the regression to Issue #394.
