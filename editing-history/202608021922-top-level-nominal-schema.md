# Top-level nominal value schemas

- `cr edit schema` now accepts a fully qualified quoted nominal type such as `'app.schema/Store` for a top-level `def` value backed by `defstruct` or `defenum`.
- Centralize schema write validation and parsing so tag leaves, quoted builtin symbols, and named type references retain their intended annotations.
- Keep unqualified custom names rejected at the CLI boundary; a stored top-level nominal schema must include its namespace.
- Add schema validation and binary/text round-trip coverage, then verify the command on Calcit and Respo temporary Snapshot copies.
