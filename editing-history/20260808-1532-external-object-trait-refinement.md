# Refine external object traits in host FFI RFC

- Rewrote RFC section 6 around existing `deftrait` member syntax: `:field Type` tag members express field capabilities (read via tag access), `.method FnSchema` members express method capabilities (invoke via receiver-preserving JS method call).
- Replaced the separate `:ffi :members` field/method map with a minimal optional `:names` override that only maps Calcit member names to host names when they differ; member kind/type stays in the trait schema.
- Noted that `CalcitTrait` currently flattens tags and methods into `EdnTag`; implementing external objects requires extending the member descriptor to `{name, kind, type}` rather than maintaining a parallel schema.
- Kept `cirru.edn` fenced blocks for all `CodeEntry`/schema-shaped examples; `cr docs check-md` still passes all 13 blocks.
