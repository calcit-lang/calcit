# Skip transitive copies of the project package

- A project's direct dependency can reintroduce that same package through a
  transitive dependency. Current namespace collision checks then rejected the
  project source during normal compile and static-analysis commands.
- Added a project-level merge path that drops only namespaces belonging to the
  root package before applying the existing strict module merge. Direct module
  conflict checks remain unchanged.
- Added a regression test and verified the real respo-markdown project against
  the current Respo dependency graph.
