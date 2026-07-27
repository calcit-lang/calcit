# Map-to-Record Field Validation and Nil Fill

## Problem
Map-to-record rewrite was passing through ALL map keys without validation, and emitting only
the keys present in the map. This caused:
1. Maps with keys not in the struct (e.g., `:x`) being incorrectly rewritten to records
2. "fields size does not match" runtime error when the map has fewer keys than struct fields

## Fix (preprocess.rs)
1. **Field validation**: Check each map key against `struct_def.fields`. If any key is not a
   valid struct field, skip the rewrite entirely (stay as map).
2. **Nil fill**: Emit ALL struct fields in definition order. For fields not present in the map,
   emit `Calcit::Nil`. This ensures the record always has the correct field count.
3. **HashMap tracking**: Build a `HashMap<EdnTag, &Calcit>` of provided fields for O(1) lookup
   during emission.

## Key Types
- `struct_def.fields` is `Vec<EdnTag>` — compare with `EdnTag` directly, not `Arc<str>`
- `provided_fields` is `HashMap<EdnTag, &Calcit>` — avoids the `Arc<str>: Borrow<EdnTag>` issue

## Respo DomProps Design
- 29 fields with `(:: :optional <type>)` annotations
- Strings: class-name, id, type, href, src, placeholder, name, title, data-name, data-comp, target
- Dynamic: style (map), value, inner-text
- Bools: disabled, checked, spell-check, read-only, selected
- Number: tab-index
- Fns: on-click, on-input, on-focus, on-blur, on-keydown, on-keyup, on-change
- Maps: on, event
- `create-element`/`create-list-element` convert record props to map via `&record:to-map`
