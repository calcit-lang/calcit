# Required Struct field access

## Background

Field syntax previously changed its contract according to inferred receiver
type: `(:field value)` returned the declared payload for a known Struct, but
silently lowered to Option-producing `get` when the receiver looked like a Map
or lost type information. This made local edits unstable: adding or losing a
type annotation changed downstream control flow without changing source syntax.

Issue #325 clarified that dynamic container absence and required model fields
are different boundaries. AI-assisted edits need those boundaries to remain
visible so diagnostics guide code toward a declared model instead of allowing
`Dynamic`/`Option` patches to spread.

## Contract

- `(:field value)` is required access. The receiver must be a statically known
  named Struct and the field must be declared. Its result is the declared field
  type.
- `get` and `get-in` are explicit partial lookup APIs for Map and indexed/path
  access. They return nominal `Option<T>`.
- `get` on a Struct is rejected with a diagnostic pointing to required field
  syntax; the runtime also rejects dynamically hidden Struct receivers.
- Loose/anonymous Struct field access is rejected until an expected named
  Struct type rewrites or narrows it.
- Low-level `&struct:get` remains available for internal dynamic boundaries
  such as reusable trait implementation bodies.

## Type inference details

- `update` preserves the receiver's collection/Struct type.
- `&struct:from-map` preserves the Struct definition supplied as its first
  argument.
- Struct pattern-match expansion is exempt from source field validation because
  all nominal branches are preprocessed before runtime guards select a variant.

## Diagnostics

- `W_REQUIRED_STRUCT_FIELD_TYPE`: required field syntax lacks a named Struct
  contract.
- `W_UNKNOWN_STRUCT_FIELD`: a named Struct does not declare the requested field.
- `W_STRUCT_FIELD_OPTIONAL_LOOKUP`: `get` is used on a Struct.

Tests cover typed access, Map fallback rejection, unknown fields, `get` misuse,
nominal type preservation, loose Struct access, and the existing Struct suite.
