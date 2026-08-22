# 2026-08-23 Fix cross-package namespace merge validation

- Restrict same-package cycle preservation to modules with matching package identity.
- Preserve conflict errors for cross-package modules that claim a target namespace.
- Avoid rebuilding the target namespace prefix inside the merge loop.
