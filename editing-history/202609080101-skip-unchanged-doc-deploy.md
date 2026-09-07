# Skip unchanged documentation deploys / 跳过未变化的文档部署

## Summary / 概要

- The main test workflow now fetches enough Git history to compare the pushed range.
- The rsync documentation deployment runs only when a main push actually changes `docs/`.
- Pull requests and source/version-only main pushes still run all build and test gates, but no longer consume remote documentation storage or fail because an unrelated deploy target is full.

## Behavior / 行为

Documentation changes continue to exercise the existing deployment step, so remote authentication, capacity, and rsync failures remain visible when deployment is relevant. An initial push with an all-zero `before` SHA conservatively deploys.
