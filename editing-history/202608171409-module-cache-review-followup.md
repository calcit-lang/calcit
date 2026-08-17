# Module cache review follow-up

- Added a shared inter-process metadata lock for cache revision metadata updates and cleanup scans, preventing concurrent `caps` processes from losing the highest observed SemVer reference.
- Made project-view installation transactional across module links and `caps-state.cirru`; registration failures now restore the previous view and state before returning an error.
- Added concurrency and rollback regression tests, and routed module documentation tests through project-root-aware resolver helpers so the production path is covered.
