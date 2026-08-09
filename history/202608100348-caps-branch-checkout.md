## caps remote branch checkout

- `caps` now fetches remote branch refs as well as tags when resolving an
  existing module clone.
- If a requested branch exists only as `origin/<branch>`, checkout now creates
  a local tracking branch instead of failing after the fetch succeeds.
- This keeps branch-based module integration reproducible through `caps
  download`, without manually editing the module cache.
