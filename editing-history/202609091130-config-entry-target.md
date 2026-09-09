# Support entry target in config set/show

- Added `calcit config set [--entry <name>] target <browser|node|native|wasm>` so projects can migrate entry targets through the supported structured Snapshot writer.
- Made `calcit config show` report every parsed entry target and mark omitted targets as `(none)`.
- Added default/named entry round-trip coverage for all four targets plus a regression assertion that invalid targets leave the Snapshot unchanged.
- Updated CLI help and entry configuration documentation with the structured target workflow.
