# Preserve nominal definition identity and applied arguments

## Context

Milestone issue #843 requires the type checker to distinguish declarations by
qualified identity, retain applied ADT arguments, and invalidate stale schema
evidence without treating `impl-traits` decoration as a new data type. This
change also carries the valid directionality finding that arrived on merged PR
#891 after its merge.

## Changes

- Added an optional qualified `definition_ref` to struct and enum definitions.
  Program storage and source-code resolution attach the owning
  `namespace/definition` while the legacy serialized struct-prototype shape is
  unchanged.
- Added a shared nominal comparison rule based on declaration path and data
  schema. Attached impls are deliberately excluded, so decoration retains the
  base data identity; changed fields, payloads, generics, or bounds invalidate
  old evidence.
- Routed Struct, Enum, TypeRef, definition-value, and instance-value proof
  cases through nominal identity. Strict proof rejects cross-namespace
  short-name matches and treats missing identity or erased generic arguments as
  explicit boundaries.
- Made applied nominal arguments participate in ordering, and updated runtime
  definition comparison/hash ordering to include qualified definition identity.
- Propagated qualified paths through struct/enum resolution and data-shape
  derivation. Data-shape validation accepts clones of the same qualified schema
  but rejects a stale incompatible reload.
- Split the directional TypeSlot/TypeRef proof fallbacks and TypeRef-to-ADT
  argument relations reported by CodeRabbit on PR #891. Added the concrete
  AnonymousEnum-against-expected-TypeSlot regression.
- Added coverage for same-name declarations in different namespaces, changed
  schema at the same path, impl decoration, bare/applied generic mismatch,
  nested List arguments, and runtime program identity attachment.

## Verification

- `cargo test --workspace --all-features` (727 library + 304 native binary +
  23 WASM binary tests)
- `cargo test -q type_inference --lib`
- focused nominal identity, data-shape reload, ordering/hash, program storage,
  and PR #891 directionality regressions
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `fnm exec --using=24.4.1 yarn check-all` (complete native, generated JS, IR,
  WASM, agent-interface, literal-path, and quality gates)
