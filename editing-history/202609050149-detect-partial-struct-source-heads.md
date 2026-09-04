# Detect partial Struct source heads

## Context

The strict partial-Struct check used `grab_def_name` for symbolic call heads,
but that helper returns the enclosing definition name rather than the called
symbol. A source `%{}?` macro could therefore reach expansion before
`E_PARTIAL_STRUCT_NIL_FILL`, while the already-lowered native form was caught.

## Change

- Reuse the resolved core-head identity check before macro expansion and match
  both `%{}?` and `&%{}?` alongside the native partial-Struct proc.
- Exercise both source spellings through the parser in strict mode.
- Document both legacy spellings and the complete `%{}` / explicit `Option<T>`
  migration in the agent guide, Struct guide, and quick reference.

Compatibility mode keeps the legacy nil-filling behavior; strict mode changes
only the previously missed source-head path.
