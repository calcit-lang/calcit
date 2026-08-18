# Document `decode-map-as` in calcit-core

## Change summary

- Added the `decode-map-as` runtime syntax to the core snapshot metadata with builtin/internal/meta/syntax tags and a typed boundary schema.
- Added reusable `RuntimeMapMeta` and `RuntimeMapResponse` Struct fixtures for core examples and tests.
- Added a passing example plus definition-attached unit tests covering recursive Struct decoding through an Option field, omitted `Option` fields becoming `%none`, pre-wrapped `%some`, and explicit Dynamic payload preservation.
- Documented the native/JavaScript support boundary and kept WASM behavior explicit.

## Knowledge point

Compiler syntax and core snapshot metadata are separate surfaces. Adding a Rust/JS builtin is incomplete until `calcit-core.cirru` exposes its documentation, examples, schema, and tests; otherwise `cr docs`, `cr test`, and downstream agents cannot discover or verify the feature.
