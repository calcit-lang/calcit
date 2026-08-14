# Typed Option membership and method-style guidance

- The nominal-enum legacy-absence warning now permits `includes?` / `contains?`
  when the candidate and the checked collection element are statically the same
  nominal enum. This keeps `Set<Option<T>>` membership type-safe without
  treating an `Option` as a nullable payload.
- Added a regression test covering `includes?` over `Set<Option<Number>>`.
- Updated Option/Result user-facing documentation to prefer receiver methods
  such as `.and-then`, `.or-else`, `.map`, and `.unwrap-or`; direct
  `option:*` / `result:*` helpers remain implementation-level compatibility
  details rather than the recommended public style.
