# Type quality and JS FFI documentation hints

- Added a direct `cr docs read library-quality.md --full` hint when the native
  `analyze quality` gate fails, so baseline regressions lead to the accepted
  CI workflow rather than project-local report scripts.
- Added a direct `cr docs read js-interop.md --full` hint to strict JS FFI
  capability, target, and read-only external-field diagnostics.
- Added regression assertions that the strict FFI errors retain their stable
  diagnostic code and expose the documentation command.

Validation: targeted Rust tests for the affected diagnostics, full Rust tests,
Markdown documentation checks, and `git diff --check`.
