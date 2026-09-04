# Recurse through function nominal contracts

## Context

Follow-up review of PR #638 noted that nominal types nested in callback
contracts were not included in the open-to-closed boundary comparison.

## Changes

- Detect nominal contracts inside TypeRef arguments and function required,
  rest, and return positions.
- Compare matching function contracts recursively so
  `Fn(Dynamic)->Unit` cannot enter `Fn(Person)->Unit` in strict project source.
- Cover callback and unresolved TypeRef nominal nesting in the focused test.

## Validation

- Focused strict boundary regression passes.
- Clippy with warnings denied passes.
- Full repository gates are rerun before push.
