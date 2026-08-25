# Macro parameter diagnostics and coverage

- Added one shared parameter-shape model for required, optional, and rest bindings, including malformed marker sequences. Both preprocessing and `analyze check-types` now use the same comparison and stable diagnostic codes.
- Preserved ordinary function compatibility: existing function schemas describe fixed arity without separately encoding the `?` marker. Strict optional-shape comparison is enabled for macros, while ordinary functions compare total fixed arity.
- Macro schema-shape mismatches remain staged during ecosystem migration. `analyze check-types` reports them by default; location-aware preprocessing warnings can be enabled with `CALCIT_WARN_MACRO_SCHEMA_SHAPE=1` and use `W_MACRO_SCHEMA_PARAM_SHAPE`.
- Corrected whole-`Dynamic` macro coverage from `Full` to `None`; explicitly typed macro schemas with Dynamic argument/result slots remain `Partial`. Explicit `:: Dynamic` now remains distinct from an omitted schema across binary serialization and reports `W_MACRO_SCHEMA_DYNAMIC`, while omission reports `W_SCHEMA_MISSING`.
- Same-command coverage baseline:
  - Calcit core: overall `full/partial/none` changed from `420/107/18` to `405/107/33`; macros changed from `18/61/0` to `3/61/15`.
  - Respo `respo.core`: overall changed from `12/53/1` to `7/53/6`; its five whole-`Dynamic` macros changed from `Full` to `None`.
- External regression used latest Respo 0.16.86 and Recollect 0.0.34. Respo required rebuilding `.calcit/modules` with `caps --ci` because its local `js-ffi` view was still at 0.1.9 despite `deps.cirru` declaring 0.1.10.
