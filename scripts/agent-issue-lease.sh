#!/usr/bin/env bash

set -euo pipefail

remote="${AGENT_LEASE_REMOTE:-origin}"
default_ttl="${AGENT_LEASE_TTL_MINUTES:-45}"
lock_prefix="refs/heads/agent-lock/issue-"

usage() {
  cat <<'EOF'
Usage:
  scripts/agent-issue-lease.sh init
  scripts/agent-issue-lease.sh claim ISSUE AGENT_ID [SCOPE] [TTL_MINUTES]
  scripts/agent-issue-lease.sh heartbeat ISSUE AGENT_ID [TTL_MINUTES]
  scripts/agent-issue-lease.sh release ISSUE AGENT_ID [ready|review|blocked]
  scripts/agent-issue-lease.sh status ISSUE

The remote agent-lock branch is the ownership source of truth. GitHub Issue
labels and the lease comment are a human-readable mirror of that lock.
EOF
}

die() {
  echo "ERROR: $*" >&2
  exit 1
}

validate_issue() {
  [[ "${1:-}" =~ ^[1-9][0-9]*$ ]] || die "ISSUE must be a positive integer"
}

validate_agent() {
  [[ "${1:-}" =~ ^[A-Za-z0-9._-]+$ ]] || die "AGENT_ID may contain only letters, digits, dot, underscore, and dash"
}

validate_ttl() {
  [[ "${1:-}" =~ ^[0-9]+$ ]] || die "TTL_MINUTES must be an integer"
  (( $1 >= 5 && $1 <= 240 )) || die "TTL_MINUTES must be between 5 and 240"
}

iso_time() {
  if date -u -r "$1" '+%Y-%m-%dT%H:%M:%SZ' >/dev/null 2>&1; then
    date -u -r "$1" '+%Y-%m-%dT%H:%M:%SZ'
  else
    date -u -d "@$1" '+%Y-%m-%dT%H:%M:%SZ'
  fi
}

remote_sha() {
  git ls-remote --heads "$remote" "$1" | awk 'NR == 1 { print $1 }'
}

fetch_lock() {
  git fetch --quiet "$remote" "$1"
}

field() {
  git show -s --format=%B "$1" | sed -n "s/^$2=//p" | head -n 1
}

make_lock_commit() {
  local issue="$1" agent="$2" claimed="$3" expires="$4" scope="$5" parent="${6:-}"
  local tree message
  # Lease refs must never publish the caller's repository tree or HEAD ancestry.
  # `git mktree` with empty input produces Git's stable empty tree object.
  tree="$(git mktree </dev/null)"
  message="agent-lease-v1
issue=$issue
agent_id=$agent
claimed_at=$claimed
heartbeat_at=$(date +%s)
expires_at=$expires
scope=$scope"

  if [[ -n "$parent" ]]; then
    GIT_AUTHOR_NAME="Calcit Agent Lease" \
    GIT_AUTHOR_EMAIL="agent-lease@calcit-lang.invalid" \
    GIT_COMMITTER_NAME="Calcit Agent Lease" \
    GIT_COMMITTER_EMAIL="agent-lease@calcit-lang.invalid" \
      git commit-tree "$tree" -p "$parent" <<<"$message"
  else
    GIT_AUTHOR_NAME="Calcit Agent Lease" \
    GIT_AUTHOR_EMAIL="agent-lease@calcit-lang.invalid" \
    GIT_COMMITTER_NAME="Calcit Agent Lease" \
    GIT_COMMITTER_EMAIL="agent-lease@calcit-lang.invalid" \
      git commit-tree "$tree" <<<"$message"
  fi
}

repo_slug() {
  gh repo view --json nameWithOwner --jq .nameWithOwner
}

ensure_labels() {
  local repo="$1"
  gh label create agent:ready --repo "$repo" --color 1D76DB --description "Available for an agent" --force >/dev/null
  gh label create agent:claimed --repo "$repo" --color FBCA04 --description "Held by an active agent lease" --force >/dev/null
  gh label create agent:review --repo "$repo" --color 0E8A16 --description "Implementation ready for review" --force >/dev/null
  gh label create agent:blocked --repo "$repo" --color D93F0B --description "Waiting for human input or a dependency" --force >/dev/null
}

issue_has_label() {
  gh issue view "$1" --repo "$2" --json labels --jq '.labels[].name' | grep -Fxq "$3"
}

require_claimable_issue() {
  local issue="$1" repo="$2" state
  state="$(gh issue view "$issue" --repo "$repo" --json state --jq .state)"
  [[ "$state" == "OPEN" ]] || die "issue #$issue is not open"
  if issue_has_label "$issue" "$repo" agent:ready; then
    return
  fi
  # A review Issue may be reclaimed to address failed checks or feedback.
  if issue_has_label "$issue" "$repo" agent:review; then
    return
  fi
  # A claimed label without a lock is a recoverable partial update from an
  # interrupted release. The Git ref remains the ownership authority.
  if issue_has_label "$issue" "$repo" agent:claimed; then
    return
  fi
  die "issue #$issue is neither agent:ready nor agent:review"
}

set_issue_state() {
  local issue="$1" repo="$2" target="$3"
  gh issue edit "$issue" --repo "$repo" \
    --remove-label agent:ready --remove-label agent:claimed \
    --remove-label agent:review --remove-label agent:blocked \
    --add-label "agent:$target" >/dev/null
}

upsert_comment() {
  local issue="$1" repo="$2" body="$3" marker comment_id endpoint
  marker="<!-- agent-lease:$issue -->"
  endpoint="repos/$repo/issues/$issue/comments"
  comment_id="$(gh api "$endpoint" --paginate --jq ".[] | select(.body | contains(\"$marker\")) | .id" | tail -n 1)"
  if [[ -n "$comment_id" ]]; then
    gh api --method PATCH "repos/$repo/issues/comments/$comment_id" -f body="$body" >/dev/null
  else
    gh api --method POST "$endpoint" -f body="$body" >/dev/null
  fi
}

sync_claimed_issue() {
  local issue="$1" repo="$2" agent="$3" claimed="$4" heartbeat="$5" expires="$6" scope="$7" sha="$8"
  local body
  set_issue_state "$issue" "$repo" claimed
  body="<!-- agent-lease:$issue -->
### Active agent lease

- Agent: \`$agent\`
- Claimed: $(iso_time "$claimed")
- Heartbeat: $(iso_time "$heartbeat")
- Expires: $(iso_time "$expires")
- Scope: \`$scope\`
- Lock: \`${sha:0:12}\`

This comment is maintained by \`scripts/agent-issue-lease.sh\`. The remote lock branch is authoritative."
  upsert_comment "$issue" "$repo" "$body"
}

command_init() {
  local repo
  repo="$(repo_slug)"
  ensure_labels "$repo"
  echo "INITIALIZED $repo agent labels"
}

command_claim() {
  local issue="$1" agent="$2" scope="${3:-repository scope declared in issue}" ttl="${4:-$default_ttl}"
  local ref old_sha now expires claimed old_agent old_expires new_sha repo result
  validate_issue "$issue"
  validate_agent "$agent"
  validate_ttl "$ttl"
  [[ "$scope" != *$'\n'* ]] || die "SCOPE must be one line"
  git rev-parse --is-inside-work-tree >/dev/null
  git remote get-url "$remote" >/dev/null

  ref="${lock_prefix}${issue}"
  old_sha="$(remote_sha "$ref")"
  now="$(date +%s)"
  expires="$((now + ttl * 60))"
  repo="$(repo_slug)"
  ensure_labels "$repo"

  if [[ -z "$old_sha" ]]; then
    require_claimable_issue "$issue" "$repo"
    claimed="$now"
    new_sha="$(make_lock_commit "$issue" "$agent" "$claimed" "$expires" "$scope")"
    if ! result="$(git push "$remote" "$new_sha:$ref" 2>&1)"; then
      echo "$result" >&2
      die "claim race lost; inspect the current owner with: $0 status $issue"
    fi
    sync_claimed_issue "$issue" "$repo" "$agent" "$claimed" "$now" "$expires" "$scope" "$new_sha"
    echo "CLAIMED issue #$issue by $agent until $(iso_time "$expires")"
    return
  fi

  fetch_lock "$ref"
  old_agent="$(field "$old_sha" agent_id)"
  old_expires="$(field "$old_sha" expires_at)"
  [[ "$old_expires" =~ ^[0-9]+$ ]] || die "remote lock has invalid metadata; human repair required"

  if [[ "$old_agent" == "$agent" ]]; then
    claimed="$(field "$old_sha" claimed_at)"
  elif (( old_expires <= now )); then
    claimed="$now"
    echo "WARN: taking over expired lease from $old_agent" >&2
  else
    die "issue #$issue is held by $old_agent until $(iso_time "$old_expires")"
  fi

  # A maintainer may close or block an Issue while its prior lease is still
  # present. Revalidate immediately before renewing or taking it over.
  require_claimable_issue "$issue" "$repo"
  new_sha="$(make_lock_commit "$issue" "$agent" "$claimed" "$expires" "$scope" "$old_sha")"
  git push --force-with-lease="$ref:$old_sha" "$remote" "$new_sha:$ref" >/dev/null
  sync_claimed_issue "$issue" "$repo" "$agent" "$claimed" "$now" "$expires" "$scope" "$new_sha"
  echo "RENEWED issue #$issue by $agent until $(iso_time "$expires")"
}

command_heartbeat() {
  local issue="$1" agent="$2" ttl="${3:-$default_ttl}"
  local ref old_sha old_agent claimed scope now expires new_sha repo
  validate_issue "$issue"
  validate_agent "$agent"
  validate_ttl "$ttl"
  ref="${lock_prefix}${issue}"
  old_sha="$(remote_sha "$ref")"
  [[ -n "$old_sha" ]] || die "issue #$issue has no active lock"
  fetch_lock "$ref"
  old_agent="$(field "$old_sha" agent_id)"
  [[ "$old_agent" == "$agent" ]] || die "issue #$issue is held by $old_agent, not $agent"
  repo="$(repo_slug)"
  ensure_labels "$repo"
  # Do not extend ownership after the Issue has been closed or blocked.
  require_claimable_issue "$issue" "$repo"
  claimed="$(field "$old_sha" claimed_at)"
  scope="$(field "$old_sha" scope)"
  now="$(date +%s)"
  expires="$((now + ttl * 60))"
  new_sha="$(make_lock_commit "$issue" "$agent" "$claimed" "$expires" "$scope" "$old_sha")"
  git push --force-with-lease="$ref:$old_sha" "$remote" "$new_sha:$ref" >/dev/null
  sync_claimed_issue "$issue" "$repo" "$agent" "$claimed" "$now" "$expires" "$scope" "$new_sha"
  echo "HEARTBEAT issue #$issue by $agent until $(iso_time "$expires")"
}

command_release() {
  local issue="$1" agent="$2" target="${3:-review}"
  local ref old_sha old_agent repo body
  validate_issue "$issue"
  validate_agent "$agent"
  [[ "$target" =~ ^(ready|review|blocked)$ ]] || die "release state must be ready, review, or blocked"
  ref="${lock_prefix}${issue}"
  old_sha="$(remote_sha "$ref")"
  [[ -n "$old_sha" ]] || die "issue #$issue has no active lock"
  fetch_lock "$ref"
  old_agent="$(field "$old_sha" agent_id)"
  [[ "$old_agent" == "$agent" ]] || die "issue #$issue is held by $old_agent, not $agent"

  repo="$(repo_slug)"
  ensure_labels "$repo"
  git push --force-with-lease="$ref:$old_sha" "$remote" ":$ref" >/dev/null
  # Delete the authoritative lock before updating its human-readable mirror.
  # If the Issue update is interrupted, the next claim can repair a stale label;
  # the inverse ordering could advertise review while a write lease still exists.
  set_issue_state "$issue" "$repo" "$target"
  body="<!-- agent-lease:$issue -->
### Agent lease released

- Last agent: \`$agent\`
- Released: $(iso_time "$(date +%s)")
- Resulting state: \`agent:$target\`

The issue currently has no active write lease."
  upsert_comment "$issue" "$repo" "$body"
  # A new claim may land after the authoritative ref deletion but before the
  # human-readable Issue mirror above is complete. Re-read the ref last and
  # repair the mirror when that race occurred; if a claim starts after this
  # check, its own synchronization naturally wins.
  local replacement_sha replacement_agent replacement_claimed replacement_heartbeat replacement_expires replacement_scope
  replacement_sha="$(remote_sha "$ref")"
  if [[ -n "$replacement_sha" ]]; then
    if ! fetch_lock "$ref"; then
      replacement_sha="$(remote_sha "$ref")"
      if [[ -z "$replacement_sha" ]]; then
        echo "WARN: the newer lease disappeared during mirror repair; its release owns the final Issue state" >&2
        echo "RELEASED issue #$issue by $agent to agent:$target"
        return
      fi
      die "newer lease $replacement_sha still exists but could not be fetched for mirror repair"
    fi
    # The ref may advance after the initial ls-remote. Read metadata from the
    # commit that fetch_lock actually fetched, never from the stale observation.
    replacement_sha="$(git rev-parse FETCH_HEAD)"
    replacement_agent="$(field "$replacement_sha" agent_id)"
    replacement_claimed="$(field "$replacement_sha" claimed_at)"
    replacement_heartbeat="$(field "$replacement_sha" heartbeat_at)"
    replacement_expires="$(field "$replacement_sha" expires_at)"
    replacement_scope="$(field "$replacement_sha" scope)"
    if [[ -n "$replacement_agent" && -n "$replacement_scope" && "$replacement_claimed" =~ ^[0-9]+$ && "$replacement_heartbeat" =~ ^[0-9]+$ && "$replacement_expires" =~ ^[0-9]+$ ]]; then
      sync_claimed_issue \
        "$issue" "$repo" "$replacement_agent" "$replacement_claimed" "$replacement_heartbeat" \
        "$replacement_expires" "$replacement_scope" "$replacement_sha"
      echo "WARN: a newer lease appeared during release; repaired the Issue mirror from $replacement_sha" >&2
    else
      set_issue_state "$issue" "$repo" claimed
      echo "WARN: a newer lease appeared during release with invalid metadata; preserved agent:claimed for human repair" >&2
    fi
  fi
  echo "RELEASED issue #$issue by $agent to agent:$target"
}

command_status() {
  local issue="$1" ref sha now agent expires claimed heartbeat scope state
  validate_issue "$issue"
  ref="${lock_prefix}${issue}"
  sha="$(remote_sha "$ref")"
  if [[ -z "$sha" ]]; then
    echo "UNLOCKED issue #$issue"
    return
  fi
  fetch_lock "$ref"
  agent="$(field "$sha" agent_id)"
  expires="$(field "$sha" expires_at)"
  claimed="$(field "$sha" claimed_at)"
  heartbeat="$(field "$sha" heartbeat_at)"
  scope="$(field "$sha" scope)"
  if [[ ! "$claimed" =~ ^[0-9]+$ || ! "$heartbeat" =~ ^[0-9]+$ || ! "$expires" =~ ^[0-9]+$ ]]; then
    printf 'CORRUPT issue #%s\nagent=%s\nclaimed_at=%s\nheartbeat_at=%s\nexpires_at=%s\nscope=%s\nlock=%s\n' \
      "$issue" "$agent" "$claimed" "$heartbeat" "$expires" "$scope" "$sha"
    return 1
  fi
  now="$(date +%s)"
  state="ACTIVE"
  if [[ "$expires" =~ ^[0-9]+$ ]] && (( expires <= now )); then
    state="EXPIRED"
  fi
  printf '%s issue #%s\nagent=%s\nclaimed=%s\nheartbeat=%s\nexpires=%s\nscope=%s\nlock=%s\n' \
    "$state" "$issue" "$agent" "$(iso_time "$claimed")" "$(iso_time "$heartbeat")" \
    "$(iso_time "$expires")" "$scope" "$sha"
}

command="${1:-}"
case "$command" in
  init)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    command_init
    ;;
  claim)
    (( $# >= 3 && $# <= 5 )) || { usage >&2; exit 2; }
    command_claim "$2" "$3" "${4:-}" "${5:-$default_ttl}"
    ;;
  heartbeat)
    (( $# >= 3 && $# <= 4 )) || { usage >&2; exit 2; }
    command_heartbeat "$2" "$3" "${4:-$default_ttl}"
    ;;
  release)
    (( $# >= 3 && $# <= 4 )) || { usage >&2; exit 2; }
    command_release "$2" "$3" "${4:-review}"
    ;;
  status)
    [[ $# -eq 2 ]] || { usage >&2; exit 2; }
    command_status "$2"
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
