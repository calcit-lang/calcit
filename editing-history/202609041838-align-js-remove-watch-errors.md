# Align JavaScript remove-watch errors

PR review identified a backend mismatch after the watcher type contracts were tightened. Native `remove-watch` raises when the requested tag is absent, while the JavaScript implementation ignored the boolean result from `Map.delete`.

The JavaScript backend now raises the same stable missing-key message when deletion returns false. The shared Calcit ref test covers the absent-key path, so both native and generated JavaScript execute the same assertion.

Validation for this follow-up includes formatting, TypeScript compilation, native core execution, generated JavaScript execution, strict Clippy, serialized Rust tests, and the repository-wide `check-all` gate.
