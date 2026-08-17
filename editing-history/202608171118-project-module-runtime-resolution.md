# Project module runtime resolution

- `caps` already stores immutable dependency revisions in `~/.config/calcit/modules/.store/` and builds the project's `.calcit/modules/` link view.
- Runtime loaders must consume only that project view. Retaining a global module fallback can silently run a dependency revision that was not selected by the project's dependency graph.
- Centralize the project module directory calculation in `calcit::project_module_folder`, then use it for normal execution, WASM codegen, queries, configuration inspection, markdown checks, call-graph comparison, and the Cirru integration harness.
- Keep explicitly relative and absolute module paths working; only package-style module paths are constrained to the project view.
