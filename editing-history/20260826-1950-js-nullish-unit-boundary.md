# Keep JS nullish checks aligned with `&unit`

- Treat both `nil` (JavaScript `null`) and `&unit` (JavaScript `undefined`) as
  nullish at `JsNullish<T>` boundaries.
- Define `js-present?` as the complement of `js-nullish?` so the two helpers
  cannot drift apart again.
- Add examples covering both runtime representations. This prevents optional
  host fields whose value is `undefined` from being dereferenced as present.
