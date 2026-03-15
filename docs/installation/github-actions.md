# GitHub Actions

To load Calcit `0.9.18` in a Ubuntu container:

```yaml
- uses: calcit-lang/setup-cr@0.0.8
  with:
    version: "0.9.18"
```

Latest release could be found on https://github.com/calcit-lang/setup-cr/releases/ .

Then to load packages defined in `deps.cirru` with `caps`:

```bash
caps --ci
```

The JavaScript dependency lives in `package.json`:

```js
"@calcit/procs": "^0.9.18"
```

Up to date example can be found on https://github.com/calcit-lang/respo-calcit-workflow/blob/main/.github/workflows/upload.yaml#L11 .
