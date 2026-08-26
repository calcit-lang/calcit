# Prune cold core macros and compatibility stubs

- Removed 23 unused or superseded macros from the bundled core, reducing the audited macro inventory from 86 to 63: `\\.`, `%<-`, `<-`, `[,]`, `[][]`, `thread-as`, `thread-first`, `thread-last`, `call-w-log`, `call-wo-log`, `defn-w-log`, `defn-wo-log`, `field-match`, `calcit.internal/&field-match-internal`, `first-or`, `last-or`, `nth-or`, `get-or`, `get-in-or`, `get-env-or`, `result:let`, `record-match`, and `record-with`.
- Removed the already non-functional `record-struct` and `record?` compatibility stubs, plus their dedicated migration-diagnostic cases. Generic legacy `&record:*` and tuple diagnostics remain until their lower-level compatibility removal is handled separately.
- Use `->`, `->>`, and `->%` instead of their aliases. Reverse the form order explicitly when migrating `<-` or `%<-`.
- Use ordinary `[]` construction instead of `[,]`; build a list of lists explicitly instead of `[][]`; write explicit nested `fn` forms instead of `\\.`.
- Use `fn`/`defn`, `w-log`/`wo-log`, or explicit logging instead of the removed call/definition logging wrappers.
- Call `.unwrap-or` on the `Option` returned by `get`, `get-in`, `get-env`, `first`, `last`, or `nth` instead of the six `*-or` macros.
- Chain Result values with receiver-first `.and-then` instead of `result:let`. `option:let` remains because current Respo code uses it.
- Replace `field-match` with an explicit field read followed by `case`/`match`; use `struct-match` for nominal structs. Replace the removed record names with `struct-match`, `struct-with`, `struct-definition`, `struct?`, or `struct-def?` as appropriate.
- Migrated the Calcit runtime fixtures and current documentation to the canonical forms. Historical RFCs are retained with explicit withdrawn/partial status notes; editing-history records remain unchanged.
