# Fix foldr-shortcut contract

## Context

The bundled Snapshot declared only three arguments for `foldr-shortcut`, while
the native runtime requires four: list, initial accumulator, exhaustion
default, and reducer. Its accumulator/result relationship was also erased by
Dynamic.

## Changes

- Declare `foldr-shortcut` as
  `List<T> × U × U × Fn(U,T)->Enum -> U`.
- Correct the documentation to describe the four-argument contract and the
  anonymous boolean decision enum.
- Add definition-attached tests for shortcut and exhaustion/default paths.
- Lower the core quality baseline from 303 to 301 unresolved schema-Dynamic
  positions.

The callback's anonymous enum payload cannot yet express its relationship to
`U`; a future nominal fold-decision type may close that remaining validation
gap without pretending that arbitrary enums are sufficient.

## Validation

- `calcit ... test calcit.core/foldr-shortcut`: 2/2.
- Bundled-core unit suite and quality baseline.
- Full repository gates after the Snapshot update.
