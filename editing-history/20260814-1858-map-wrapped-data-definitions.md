# Map-wrapped data-definition macro forms

- `defstruct Name $ {}` and `defenum Name $ {}` deliver one map-headed AST
  wrapper through their variadic macro parameters. Normalize that wrapper into
  the contained field or variant forms before detecting generics and where
  bounds.
- Compare the macro AST head with quoted `'{}`; evaluating `{}` would instead
  create a runtime map and fails to recognize the syntax marker.
- Added definition-attached end-to-end fixtures for typed Struct field access
  and Enum construction/tag matching, so both wrapper forms remain covered by
  the native test suites.
