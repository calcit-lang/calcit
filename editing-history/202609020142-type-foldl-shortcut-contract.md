# Type foldl-shortcut contract

## Context

The bundled Snapshot still declared `foldl-shortcut` as four unrelated
Dynamic arguments and a Dynamic result. Its documentation also described only
three arguments even though every backend requires list, initial accumulator,
exhaustion default, and reducer.

## Changes

- Declare `foldl-shortcut` as
  `List<T> × U × U × Fn(U,T)->Enum -> U`.
- Correct the documentation to describe the four-argument contract and the
  anonymous boolean decision enum.
- Add definition-attached tests for the left-to-right shortcut result and the
  exhaustion/default path.
- Lower the core quality baseline from 301 to 297 unresolved schema-Dynamic
  positions.

As with `foldr-shortcut`, the callback uses broad `Enum` because the current
schema language cannot relate an anonymous enum payload to `U`. A future
nominal fold-decision type can close that remaining modeling gap.

## Validation

- `calcit ... test calcit.core/foldl-shortcut`: 2/2.
- Bundled-core quality baseline regenerated after reviewing the definition.
- Full repository gates run after the Snapshot update.
