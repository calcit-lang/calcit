# Release manifest review follow-up

The release-manifest helper now emits a single-line usage error instead of a
stack trace. The publishing workflow passes the release tag through an
environment variable, preventing release metadata from being interpolated into
shell source.
