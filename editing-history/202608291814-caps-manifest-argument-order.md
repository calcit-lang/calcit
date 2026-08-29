# Preserve `caps` manifest argument order in mutation guidance

The custom `deps.cirru` path is a top-level `caps` positional argument. Guidance must use `caps <deps-file> upgrade --all`; placing the path after `upgrade --all` makes argh parse it as a dependency package name. The mismatch regression test now locks the correct ordering.
