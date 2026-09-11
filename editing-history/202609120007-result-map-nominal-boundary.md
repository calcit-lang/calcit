# Cover the Result reference boundary

- Keep the short `Result<T,E>` reference accepted because embedded core API schemas still preserve that form during preprocessing.
- Add a regression proving that an explicitly user-qualified `Result` reference does not acquire the core `result:map` contract.
- Retain exact definition identity for resolved enum values, covering the nominal boundary without restoring the `fs:path` warnings.
