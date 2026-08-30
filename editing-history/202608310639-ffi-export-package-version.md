# FFI export package version / FFI 导出项目版本

- `calcit ffi export` now reads `package_version` from the adjacent
  `deps.cirru :version`, matching the project release source of truth.
- Projects without a manifest version retain the legacy snapshot fallback;
  malformed manifest values fail with a deterministic error.
- Added focused tests and documented the version-source contract.

- `calcit ffi export` 现在从相邻 `deps.cirru :version` 读取
  `package_version`，与项目发版的事实来源保持一致。
- 未声明 manifest 版本的旧项目继续回退到 snapshot；非法值会确定性报错。
- 增加针对性测试，并记录版本来源契约。
