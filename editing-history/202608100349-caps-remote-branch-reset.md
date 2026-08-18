## caps remote branch checkout hardening

- Existing module clones can use narrow or non-standard remote refspecs, for
  which `git checkout --track origin/<branch>` may reject an otherwise fetched
  remote ref.
- `caps` now creates/resets the requested local branch with `git checkout -B
  <branch> origin/<branch>`, keeping branch-based module downloads reliable and
  compatible with later `--pull-branch` updates.
