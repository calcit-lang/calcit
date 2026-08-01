# Store entry functions as Calcit symbols

- `:init-fn` and `:reload-fn` describe namespace/definition references, so canonical snapshot output now writes them as Cirru EDN symbols such as `'app.main/main!`.
- Snapshot readers continue to accept the prior string form (`|app.main/main!`) for compatibility.
- The embedded core-snapshot build model accepts either representation when deserializing entry metadata.
- Reinstalled `cr` globally and used it to rewrite all `calcit/**/*.cirru` snapshots; 120 entry function references now use symbols.
- Updated snapshot documentation and round-trip coverage to make the canonical syntax explicit.
