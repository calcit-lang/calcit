# Module cache review follow-up

- Scope `cr docs` module documentation and scope discovery to the current project's `.calcit/modules/` directory, removing the final global fallback.
- Register each successfully materialized project module view in `module-caches/projects/`; global cleanup preserves every cached revision that remains linked by one of those views.
- When a cached commit is later resolved through a SemVer tag, refresh its metadata with the highest observed SemVer ref so cleanup ordering stays correct.
