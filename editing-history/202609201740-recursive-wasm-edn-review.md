# Recursive WASM EDN review fixes

The recursive token scanner is shared by List items and Map/Struct entry
values. It tracks quoted regions and escapes so parentheses and whitespace in a
quoted String never change the surrounding collection depth.

Map keys are validated against the same scalar subset that the formatter can
order. Rejecting recursive key shapes before code generation keeps parsing and
formatting symmetric and avoids accepting values that cannot round-trip.

Verification includes the existing Map WASI smoke plus a nested quoted-string
roundtrip through Wasmtime.
