# Reject implicit Dynamic in bare container schemas

- Added strict preprocessing diagnostic `E_BARE_CONTAINER_SCHEMA` for public
  `List`, `Map`, `Set`, and `Ref` contracts whose omitted type arguments use the
  shared missing-schema `Dynamic` marker.
- Preserved explicitly written `List<Dynamic>` and equivalent shapes as visible,
  auditable open boundaries; the schema parser's distinct `Arc` identity makes
  this distinction possible without changing snapshot serialization.
- Diagnostics identify the first precise nested schema path and recommend
  concrete arguments, generics, or an explicit reviewed Dynamic boundary.
- Added nested-container, compatibility-mode, location, and explicit-Dynamic
  regression coverage.
- Restored the post-merge verification baseline by updating the Agent
  `query.type-at` smoke to schema v2 and applying two Rust 1.97 Clippy-only
  mechanical cleanups; neither cleanup changes runtime behavior.
