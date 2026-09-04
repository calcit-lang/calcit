# Make Dynamic generic binding transactional

## Context

Final review of PR #638 identified two remaining consistency gaps: Variadic
containers were classified as open but not compared recursively, and a failed
non-Dynamic match could leave partial generic bindings that misclassified a
later Dynamic argument.

## Changes

- Compare matching `Variadic` contracts recursively.
- Collect each argument's generic bindings in a candidate map and commit them
  only when the complete argument type matches its expected contract.
- Cover `Variadic<Dynamic>` and a Map mismatch that must not leak a partially
  bound nominal generic into the following Dynamic argument.

## Validation

- Focused strict boundary regression passes.
- Clippy with warnings denied passes.
- Full repository gates are rerun before push.
