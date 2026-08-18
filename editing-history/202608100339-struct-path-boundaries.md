## Struct path API boundaries

- Public collection path APIs (`get-in`, `assoc-in`, `update-in`, `dissoc-in`)
  now stop at nominal Struct boundaries. Required fields remain visible as direct
  `(:field value)` accesses, preserving their declared types for the checker.
- The preprocessor diagnoses literal and dynamic paths that would enter a Struct;
  `get-in` no longer infers a field type through such a path.
- Runtime core implementations reject the same traversal after a Dynamic boundary,
  so unsafe coercion cannot silently bypass the typed field-access contract.
- Updated the type-inference fixture to use nested direct field access instead of
  `get-in` across a Struct.
