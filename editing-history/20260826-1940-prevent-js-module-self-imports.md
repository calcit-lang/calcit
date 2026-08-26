# Prevent generated JavaScript module self-imports

- Keep resolved definitions from the current namespace local even when a
  type-directed rewrite conservatively marks them as referred imports.
- Add a codegen regression test proving that the local definition remains a
  direct identifier and does not enter the ESM import collection.
- Validated with skir's same-namespace `Request` and `Response` struct
  constructors, which previously produced duplicate declaration errors.
