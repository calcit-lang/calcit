# Recurse through applied nominal Dynamic arguments

## Context

Incremental review of PR #638 found that open type arguments on an otherwise
matching nominal base, such as `Box<Dynamic>` entering `Box<Person>`, were
classified as open but not rejected by the boundary comparison.

## Changes

- Recurse through applied arguments when Struct definitions, Enum definitions,
  or TypeRef names match and have the same arity.
- Keep different nominal bases outside this focused rule; ordinary type
  mismatch diagnostics continue to own those cases.
- Cover Struct, Enum, and TypeRef applied Dynamic arguments.

## Validation

- Focused strict Dynamic nominal boundary regression passes.
- Full repository gates are rerun before push.
