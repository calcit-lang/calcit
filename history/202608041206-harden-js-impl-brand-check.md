# Harden JS impl brand check

- `CalcitImpl[Symbol.hasInstance]` now reads its shared brand from an own property descriptor, so inherited brands cannot spoof an implementation value.
- The check also verifies the own implementation fields needed by consumers before accepting a cross-runtime value.
- The JS runtime identity regression check covers inherited and malformed branded objects.
