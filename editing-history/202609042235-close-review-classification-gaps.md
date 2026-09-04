# Close review classification gaps

- Preserve the dynamic-callable cause for `Optional<DynFn>` instead of
  describing its payload as a Dynamic value.
- Resolve bound type slots while detecting nominal Struct/Enum argument
  contracts, so Dynamic cannot bypass the strict nominal boundary.
- Add focused regression coverage and align the migration documentation with
  both classifications.
