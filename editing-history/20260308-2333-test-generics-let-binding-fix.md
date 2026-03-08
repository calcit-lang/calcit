# Test generics let binding fix

## Summary

- fixed `calcit/test-generics.cirru` after a schema-formatting edit moved `assert-type id ...` into the `let` binding section
- restored `assert-type id` to the `let` body so the binding list remains pairs only
- verified `target/debug/cr calcit/test.cirru -1` runs through again

## Notes

- this fix resolves the runtime error `expected binding of a pair`
- separate legacy fn syntax warnings still remain and need a follow-up migration pass
