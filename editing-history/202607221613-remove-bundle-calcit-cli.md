# Replace the legacy bundle CLI with a Calcit script

- Removed the deprecated `bundle_calcit` Rust binary and its release artifact.
- Added the recursive `read-dir` Calcit runtime API so filesystem workflows can be implemented in Calcit.
- Added `calcit/bundle-calcit.cirru` as a directly executable snapshot that bundles indentation-based source files into a runnable snapshot.
- Updated installation and bundle-mode documentation to use the script-based workflow.
- Verified the script against `minimal-calcit`, including loading and running the generated snapshot.
