# PR 352 assert-type direct resolution guard

- `assert-type` direct definition resolution now verifies the referenced def
  resolves to a concrete StructDef/EnumDef before treating it as a resolved type.
- Visible function or value names are kept as-is instead of being passed to
  `parse_type_annotation_form` as resolved runtime values, which could silently
  change the asserted type.
- Added `code_resolves_to_nominal_type_def` in `type_annotation.rs` reusing the
  existing `resolve_type_def_from_code` peeling logic.
- Extended the membership regression block with Map<Option<T>, V> key
  membership via `contains?`, Map<K, Option<T>> value membership via `includes?`,
  reversed-direction negatives, and specialized Map membership procs.
