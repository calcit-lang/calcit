# Quote the manifest path in mutation guidance

The Snapshot toolchain guard now renders the adjacent `deps.cirru` path with the existing shell quoting helper. A regression fixture uses a directory containing whitespace and verifies the suggested `caps '<deps-file>' upgrade --all` command remains a single shell argument. The fixture identity also combines a timestamp with an atomic sequence so parallel tests cannot share a manifest.
