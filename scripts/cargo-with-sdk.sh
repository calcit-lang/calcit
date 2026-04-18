#!/usr/bin/env bash
set -euo pipefail

if [[ "${OSTYPE:-}" == darwin* ]] && [[ -z "${SDKROOT:-}" ]] && command -v xcrun >/dev/null 2>&1; then
  export SDKROOT
  SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
fi

cargo "$@"
