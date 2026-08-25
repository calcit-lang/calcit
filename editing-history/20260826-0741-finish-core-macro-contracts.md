# Finish bundled core macro contracts

- Audited all 86 `defmacro` entries bundled in `calcit.core`, `calcit.internal`, and `calcit.test`, rather than relying only on macros reached by `calcit/test.cirru`.
- Migrated the remaining 21 legacy schemas, including aliases, Option/Result binding macros, anonymous enums, JS objects, logging helpers, deprecated compatibility macros, and open collection lookup helpers.
- Kept broad collection payloads explicitly `Dynamic` where list/string/enum/open-data behavior cannot yet be represented safely by one static contract; used concrete `String`, `Enum`, `JsObject`, `Option`, `Result`, `Nil`, and generic identity contracts where the semantics support them.
- Added a snapshot inventory test that rejects any bundled `defmacro` using a legacy whole-Dynamic schema and records the audited count.
- Confirmed the existing core corpus remains at 2,393 pure/cache-eligible expansions plus 39 declared-capability expansions, with zero legacy expansions. The migration itself does not implement caching, so it improves diagnostics and cache safety prerequisites without claiming a present speedup.
- Compared 12 alternating release `--check-only` runs on Respo against the installed 0.13.45 binary: both warm medians were 0.12s. The release binary grew from 9,586,224 to 9,586,256 bytes (+32 bytes), so this final schema pass shows neither a visible speed gain nor a material regression.
