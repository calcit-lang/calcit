# Route npm prereleases to `next` / npm 预发布使用 `next`

Issue: #680. Calcium consumer validation requires an immutable Calcit build
containing the generic-order fix that landed after 0.13.77. The selected release
sequence publishes `0.13.78-rc.1`, validates Calcium against that exact asset,
and only then publishes the final stable release.

The release workflow now branches on GitHub's immutable release `prerelease`
flag. Prereleases publish `@calcit/procs` with npm dist-tag `next`; stable
releases keep the existing default `latest` behavior. Cargo prerelease versions
remain handled by the existing crates.io publish step. This prevents an rc from
moving npm consumers on the default channel while preserving the stable path.

为解决 0.13.77 不包含泛型声明顺序修复、Calcium 又必须基于正式产物验证的循环，
本轮先发布 `0.13.78-rc.1`。GitHub prerelease 对应的 npm 包只更新 `next`，
正式 release 仍按原流程更新 `latest`，避免预发布版本影响默认安装用户。
