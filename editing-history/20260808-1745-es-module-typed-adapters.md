# ES module typed adapter conventions

- Documented ES module imports as opaque values that acquire `Fn` or external-trait evidence only through an explicit `unsafe-coerce` at a small adapter boundary.
- Established the JavaScript external-member default conversion: kebab case becomes camel case and trailing `?`/`!` are removed.
- Kept `:ffi :names` as the single per-member override for nonstandard JavaScript property keys, with bracket access preserving special keys safely.
- Added codegen coverage for the default member-name conversion and checked the RFC and JS interop examples with `docs check-md`.
