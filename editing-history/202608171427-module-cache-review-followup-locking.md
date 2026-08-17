# Module cache locking follow-up

- Replaced removable lock files with OS-managed `fs4` locks, so ownership is released when the handle or process exits on every supported platform.
- Held the cache lock across revision installation and metadata publication, and added a project-scoped lock spanning module view activation, state replacement, registration, rollback, and cleanup.
- Isolated the documentation fallback regression test against the actual `$HOME/.config/calcit/modules` layout without leaking the test environment.
