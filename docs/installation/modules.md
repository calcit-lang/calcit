---
title: "Modules directory"
scope: "core"
kind: "reference"
category: "installation"
aliases:
  - "modules directory"
  - "installed modules"
  - "module docs"
  - "caps"
entry_for:
  - "caps install"
  - "calcit docs remote-libs scan-md"
---

# Modules directory

Packages are managed with `caps` command, which wraps `git clone` and `git pull` to manage modules.

Configurations inside runtime snapshot files (`calcit.cirru`):

```cirru.edn
{}
  :entries $ {}
    :default $ {}
      :modules $ [] |memof/calcit.cirru |lilac/
```

Paths defined in `:modules` field are just loaded as files from `~/.config/calcit/modules/`, i.e. `~/.config/calcit/modules/memof/calcit.cirru`.

Modules that end with `/` are automatically suffixed with `calcit.cirru`. Retired `compact.cirru` modules are rejected with migration guidance.

To load modules in CI environments, make use of `caps --ci`.
