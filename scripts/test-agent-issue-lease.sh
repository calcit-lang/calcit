#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lease_script="$repo_root/scripts/agent-issue-lease.sh"
test_root="$(mktemp -d "${TMPDIR:-/tmp}/calcit-agent-lease-test.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

real_git="$(command -v git)"
remote_repo="$test_root/remote.git"
work_repo="$test_root/work"
fake_bin="$test_root/bin"
log_file="$test_root/gh.log"
race_marker="$test_root/race-created"
advance_marker="$test_root/race-advanced"
fetch_arm="$test_root/arm-fetch"
mkdir -p "$fake_bin"

"$real_git" init --bare --quiet "$remote_repo"
"$real_git" init --quiet "$work_repo"
"$real_git" -C "$work_repo" config user.name "Lease test"
"$real_git" -C "$work_repo" config user.email "lease-test@calcit-lang.invalid"
"$real_git" -C "$work_repo" remote add origin "$remote_repo"

cat >"$fake_bin/gh" <<'FAKE_GH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"$LEASE_TEST_LOG"
if [[ "$1 $2" == "repo view" ]]; then
  echo "calcit-lang/calcit"
elif [[ "$1 $2" == "label create" ]]; then
  :
elif [[ "$1 $2" == "issue view" ]]; then
  if [[ "$*" == *"--json state"* ]]; then
    echo "${LEASE_TEST_ISSUE_STATE:-OPEN}"
  else
    printf '%s\n' "${LEASE_TEST_ISSUE_LABELS-agent:ready}"
  fi
elif [[ "$1 $2" == "issue edit" ]]; then
  previous=""
  for arg in "$@"; do
    if [[ "$previous" == "--add-label" ]]; then
      printf 'MIRROR %s\n' "$arg" >>"$LEASE_TEST_LOG"
      if [[ "$arg" == "agent:review" && ! -e "$LEASE_TEST_RACE_MARKER" ]]; then
        touch "$LEASE_TEST_RACE_MARKER"
        "$LEASE_TEST_REAL_GIT" -C "$LEASE_TEST_WORK_REPO" push --quiet origin "$LEASE_TEST_OLD_SHA:refs/heads/agent-lock/issue-1"
      fi
      break
    fi
    previous="$arg"
  done
elif [[ "$1" == "api" ]]; then
  if [[ "$*" == *"--method POST"* && "$*" == *"Agent lease released"* ]]; then
    touch "$LEASE_TEST_FETCH_ARM"
  fi
else
  echo "unexpected fake gh invocation: $*" >&2
  exit 2
fi
FAKE_GH

cat >"$fake_bin/git" <<'FAKE_GIT'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$1" == "fetch" && -e "$LEASE_TEST_FETCH_ARM" && ! -e "$LEASE_TEST_ADVANCE_MARKER" ]]; then
  touch "$LEASE_TEST_ADVANCE_MARKER"
  "$LEASE_TEST_REAL_GIT" -C "$LEASE_TEST_WORK_REPO" push --quiet --force origin "$LEASE_TEST_NEW_SHA:refs/heads/agent-lock/issue-1"
fi
exec "$LEASE_TEST_REAL_GIT" "$@"
FAKE_GIT
chmod +x "$fake_bin/gh" "$fake_bin/git"

empty_tree="$("$real_git" -C "$work_repo" mktree </dev/null)"
make_test_lock() {
  local agent="$1" scope="$2" heartbeat="$3"
  local message
  message="agent-lease-v1
issue=1
agent_id=$agent
claimed_at=1700000000
heartbeat_at=$heartbeat
expires_at=2000000000
scope=$scope"
  GIT_AUTHOR_NAME="Lease test" GIT_AUTHOR_EMAIL="lease-test@calcit-lang.invalid" \
  GIT_COMMITTER_NAME="Lease test" GIT_COMMITTER_EMAIL="lease-test@calcit-lang.invalid" \
    "$real_git" -C "$work_repo" commit-tree "$empty_tree" <<<"$message"
}

old_sha="$(make_test_lock old-owner old-scope 1700000010)"
new_sha="$(make_test_lock new-owner new-scope 1700000020)"
release_sha="$(make_test_lock releasing-owner release-scope 1700000005)"
"$real_git" -C "$work_repo" push --quiet origin "$release_sha:refs/heads/agent-lock/issue-1"

export LEASE_TEST_LOG="$log_file"
export LEASE_TEST_RACE_MARKER="$race_marker"
export LEASE_TEST_ADVANCE_MARKER="$advance_marker"
export LEASE_TEST_FETCH_ARM="$fetch_arm"
export LEASE_TEST_REAL_GIT="$real_git"
export LEASE_TEST_WORK_REPO="$work_repo"
export LEASE_TEST_OLD_SHA="$old_sha"
export LEASE_TEST_NEW_SHA="$new_sha"
export LEASE_TEST_ISSUE_STATE=OPEN
export LEASE_TEST_ISSUE_LABELS=agent:ready

(
  cd "$work_repo"
  PATH="$fake_bin:$PATH" "$lease_script" release 1 releasing-owner review
)

grep -Fq 'Agent: `new-owner`' "$log_file"
grep -Fq 'Scope: `new-scope`' "$log_file"
if grep -Fq 'Agent: `old-owner`' "$log_file"; then
  echo "stale replacement metadata reached the Issue mirror" >&2
  exit 1
fi

# A replacement with missing required identity fields must retain the claimed
# label for human repair rather than publishing a blank owner or scope.
corrupt_sha="$(make_test_lock '' '' 1700000030)"
"$real_git" -C "$work_repo" push --quiet --force origin "$release_sha:refs/heads/agent-lock/issue-1"
rm -f "$race_marker" "$advance_marker" "$fetch_arm"
: >"$log_file"
export LEASE_TEST_OLD_SHA="$corrupt_sha"
export LEASE_TEST_NEW_SHA="$corrupt_sha"
(
  cd "$work_repo"
  PATH="$fake_bin:$PATH" "$lease_script" release 1 releasing-owner review
)
[[ "$(grep '^MIRROR ' "$log_file" | tail -n 1)" == "MIRROR agent:claimed" ]]
if grep -Fq 'Agent: ``' "$log_file" || grep -Fq 'Scope: ``' "$log_file"; then
  echo "blank replacement identity reached the Issue mirror" >&2
  exit 1
fi

# Heartbeat trusts the validated remote lock and repairs a missing Issue state
# label instead of rejecting the current owner because its mirror disappeared.
heartbeat_sha="$(make_test_lock heartbeat-owner heartbeat-scope 1700000040)"
"$real_git" -C "$work_repo" push --quiet --force origin "$heartbeat_sha:refs/heads/agent-lock/issue-1"
: >"$log_file"
export LEASE_TEST_ISSUE_LABELS=""
(
  cd "$work_repo"
  PATH="$fake_bin:$PATH" "$lease_script" heartbeat 1 heartbeat-owner
)
[[ "$(grep '^MIRROR ' "$log_file" | tail -n 1)" == "MIRROR agent:claimed" ]]
renewed_sha="$("$real_git" -C "$work_repo" ls-remote --heads origin refs/heads/agent-lock/issue-1 | awk 'NR == 1 { print $1 }')"
[[ -n "$renewed_sha" && "$renewed_sha" != "$heartbeat_sha" ]]
[[ "$("$real_git" -C "$work_repo" show -s --format=%B "$renewed_sha" | sed -n 's/^agent_id=//p')" == "heartbeat-owner" ]]

# Explicit blocked and closed Issue states remain non-renewable, and neither
# failure may advance the authoritative lock.
"$real_git" -C "$work_repo" push --quiet --force origin "$heartbeat_sha:refs/heads/agent-lock/issue-1"
export LEASE_TEST_ISSUE_LABELS=agent:blocked
if (
  cd "$work_repo"
  PATH="$fake_bin:$PATH" "$lease_script" heartbeat 1 heartbeat-owner
); then
  echo "blocked Issue heartbeat unexpectedly succeeded" >&2
  exit 1
fi
[[ "$("$real_git" -C "$work_repo" ls-remote --heads origin refs/heads/agent-lock/issue-1 | awk 'NR == 1 { print $1 }')" == "$heartbeat_sha" ]]

export LEASE_TEST_ISSUE_STATE=CLOSED
export LEASE_TEST_ISSUE_LABELS=""
if (
  cd "$work_repo"
  PATH="$fake_bin:$PATH" "$lease_script" heartbeat 1 heartbeat-owner
); then
  echo "closed Issue heartbeat unexpectedly succeeded" >&2
  exit 1
fi
[[ "$("$real_git" -C "$work_repo" ls-remote --heads origin refs/heads/agent-lock/issue-1 | awk 'NR == 1 { print $1 }')" == "$heartbeat_sha" ]]

# agent:blocked dominates conflicting claimable mirror labels for both renewal
# of an existing lock and acquisition of a new lock.
export LEASE_TEST_ISSUE_STATE=OPEN
export LEASE_TEST_ISSUE_LABELS=$'agent:ready\nagent:blocked'
if (
  cd "$work_repo"
  PATH="$fake_bin:$PATH" "$lease_script" claim 1 heartbeat-owner heartbeat-scope
); then
  echo "blocked existing-lock claim unexpectedly succeeded" >&2
  exit 1
fi
[[ "$("$real_git" -C "$work_repo" ls-remote --heads origin refs/heads/agent-lock/issue-1 | awk 'NR == 1 { print $1 }')" == "$heartbeat_sha" ]]
if (
  cd "$work_repo"
  PATH="$fake_bin:$PATH" "$lease_script" claim 2 new-owner new-scope
); then
  echo "blocked new-lock claim unexpectedly succeeded" >&2
  exit 1
fi
[[ -z "$("$real_git" -C "$work_repo" ls-remote --heads origin refs/heads/agent-lock/issue-2)" ]]

echo "agent lease release race and heartbeat recovery tests passed"
