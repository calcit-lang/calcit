# Accept core nominal short names in schema edits

- Allow the canonical core `Option<T>` and `Result<T,E>` short names emitted by schema queries to be submitted to `calcit edit schema`.
- Keep project nominal schemas namespace-qualified and validate the core nominal argument counts.
- Cover query/edit-style round trips and document the qualification rule in command help.
