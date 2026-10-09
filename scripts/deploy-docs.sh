#!/usr/bin/env bash
set -euo pipefail

: "${DEPLOY_KEY:?Missing rsync_private_key secret}"
deploy_agent_env="$(ssh-agent -s)"
eval "$deploy_agent_env"
trap 'ssh-agent -k >/dev/null 2>&1' EXIT

# Preserve escaped-newline keys accepted by the previous action.
printf '%b\n' "$DEPLOY_KEY" | ssh-add -
# Accept first-use trust, but never silently accept a changed host key.
rsync -avzr --progress \
  -e 'ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new' \
  docs/* "rsync-user@tiye.me:/web-assets/repo/${GITHUB_REPOSITORY}/docs"
echo 'status=Content synced successfully.' >> "$GITHUB_OUTPUT"
