# Release main-branch policy

- Main no longer requires pull-request protection for verified changes.
- The release flow now requires the stable version commit to be on `main`, followed by a fresh main-branch CI run before tagging and creating the release.
- Corrected the npm verification command to use the published package name, `@calcit/procs`.
