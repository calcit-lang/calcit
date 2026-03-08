# Schema generic restoration and validation

## Summary

- made snapshot schema loading strict for non-nil schemas, so malformed schema data no longer silently degrades to dynamic
- normalized loaded schema maps so legacy string keys and string `:kind` values are converted into canonical tag-based schema maps
- restored generic type variables in `src/cirru/calcit-core.cirru` where a previous migration had incorrectly replaced them with `:symbol`
- broadened wrapper schemas such as `map`, `every?`, `merge`, `max`, and `min` where the implementation dispatches across multiple shapes and a too-specific schema caused validation warnings
- aligned preprocess/CLI schema validation with current function forms by skipping macro arity details and correctly ignoring rest-binding names and optional markers

## Notes

- `yarn check-all` now passes again after the generic-schema restoration pass
- remaining legacy fn syntax warnings come from old forms still present in other snapshot content and are separate from the mistaken `:symbol` substitutions fixed here
