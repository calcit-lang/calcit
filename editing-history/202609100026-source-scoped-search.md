# Add source-scoped structural search

- Added explicit `project|core|deps|all` source selection to `query search`, preserving `all` as the compatible default.
- Attached deterministic source and package/module origins to every JSON definition and match, with explicit `leaf`, `call`, and `expr` node kinds.
- Preserved filtered cursor indexes and repeat-search source scope, including backward-compatible cursor-state loading.
- Added stable project/core/dependency/all fixture counts, bare-constructor versus zero-argument-call paths, Agent guidance, and query documentation.
