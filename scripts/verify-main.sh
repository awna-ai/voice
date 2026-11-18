#!/usr/bin/env bash
# Exit codes: 0 green, current, covered or already running; 1 failed (nothing judged);
# 2 usage; 5 red with one culprit; 6 red across several commits.
set -euo pipefail
cd "$(dirname "$0")/.."
usage() {
  echo 'Usage: scripts/verify-main.sh [--remote <name>] [-- <check command>...]' >&2
  exit 2
}
remote=origin
check=(bash scripts/ci-local.sh)
while [ "$#" -gt 0 ]; do
  case "$1" in
    --remote)
      [ "$#" -ge 2 ] && [ -n "$2" ] && [[ "$2" != -* ]] || usage
      remote=$2; shift 2 ;;
    --) shift; [ "$#" -gt 0 ] || usage; check=("$@"); break ;;
    *) usage ;;
  esac
done
git remote get-url -- "$remote" >/dev/null 2>&1 || usage
lock="$(git rev-parse --git-common-dir)/verify-main.lock"
if ! ln -s "$$" "$lock" 2>/dev/null; then
  pid=$(readlink "$lock" 2>/dev/null || true)
  if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null &&
    ps -p "$pid" -o command= | grep -q 'verify-main\.sh'; then
    echo "verify-main: already running (pid $pid)"
    exit 0
  fi
  if [ "$(readlink "$lock" 2>/dev/null || true)" = "$pid" ]; then rm -f "$lock"; fi
  if ! ln -s "$$" "$lock" 2>/dev/null; then
    pid=$(readlink "$lock" 2>/dev/null || true)
    echo "verify-main: already running (pid ${pid:-unknown})"
    exit 0
  fi
fi
scratch=
# Invoked by the EXIT trap, including signal exits.
# shellcheck disable=SC2329
cleanup() {
  if [ -n "$scratch" ]; then
    git worktree remove --force "$scratch/wt" >/dev/null 2>&1 || true
    rm -rf "$scratch"
  fi
  if [ "$(readlink "$lock" 2>/dev/null || true)" = "$$" ]; then rm -f "$lock"; fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
fail() { echo "verify-main: $*"; exit 1; }
valid_oid() { [[ "$1" =~ ^[[:xdigit:]]{40}$ || "$1" =~ ^[[:xdigit:]]{64}$ ]]; }
read_refs() {
  local refs oid ref
  refs=$(git ls-remote -- "$remote" refs/green/main refs/green/frozen) || fail "could not read $remote"
  green=; frozen=
  while read -r oid ref; do
    [ -n "$oid" ] || continue
    valid_oid "$oid" || fail "invalid oid from $remote"
    case "$ref" in refs/green/main) green=$oid ;; refs/green/frozen) frozen=$oid ;; esac
  done <<< "$refs"
}
ensure_commit() {
  git cat-file -e "$1^{commit}" 2>/dev/null || git fetch --quiet -- "$remote" "$1"
}
# A rewind between this re-fetch and the push is not caught; the lease guards only the green refs. That window is accepted.
still_on_main() {
  git fetch --quiet -- "$remote" main || fail "could not fetch $remote"
  git merge-base --is-ancestor "$head" "refs/remotes/$remote/main" ||
    fail "$remote/main no longer contains ${head:0:12} — nothing changed; run it again"
}
git worktree list --porcelain | while IFS= read -r entry; do
  case "$entry" in
    "worktree "*/verify-main.??????/wt)
      path=${entry#worktree }
      git worktree remove --force -- "$path" >/dev/null 2>&1 || true
      rmdir -- "${path%/wt}" 2>/dev/null || true
      ;;
  esac
done
git worktree prune
git fetch --quiet -- "$remote" main || fail "could not fetch $remote"
head=$(git rev-parse --verify "refs/remotes/$remote/main^{commit}") || fail "could not read $remote/main"
read_refs
if [ "$green" = "$head" ]; then
  if [ -n "$frozen" ]; then
    ensure_commit "$frozen" || fail "could not fetch freeze ${frozen:0:12}"
    if git merge-base --is-ancestor "$frozen" "$head"; then
      git push --porcelain "--force-with-lease=refs/green/frozen:$frozen" -- "$remote" :refs/green/frozen ||
        fail "could not clear the freeze at ${frozen:0:12}"
      echo "green is current: $head"
      echo "releases unfrozen (freeze at $frozen cleared)"
      exit 0
    fi
  fi
  echo "green is current: $head"
  exit 0
fi
if [ -n "$green" ]; then
  ensure_commit "$green" || fail "could not fetch green ${green:0:12}"
  git merge-base --is-ancestor "$green" "$head" || fail "green ${green:0:12} is not on $remote/main — nothing changed"
  count=$(git rev-list --count "$green..$head")
  description="$count commits since green ${green:0:12}"
else
  description='no green yet'
fi
scratch=$(mktemp -d "${TMPDIR:-/tmp}/verify-main.XXXXXX")
git worktree add --quiet --detach "$scratch/wt" "$head" || fail 'could not create scratch worktree'
passed=false
for attempt in 1 2; do
  echo "verify-main: checking ${head:0:12} ($description)"
  result=0
  (cd "$scratch/wt" && "${check[@]}") || result=$?
  if [ "$result" = 0 ]; then passed=true; break; fi
  if [ "$attempt" = 1 ]; then
    echo "verify-main: ${head:0:12} was red (exit $result); running it once more"
  fi
done
if $passed; then
  if [ "$attempt" = 2 ]; then echo "verify-main: ${head:0:12} passed on the rerun (the first run was red)"; fi
  still_on_main
  push=(--atomic --porcelain "--force-with-lease=refs/green/main:$green")
  clear=false
  if [ -n "$frozen" ]; then
    ensure_commit "$frozen" || fail "could not fetch freeze ${frozen:0:12}"
    if git merge-base --is-ancestor "$frozen" "$head"; then
      push+=("--force-with-lease=refs/green/frozen:$frozen")
      clear=true
    fi
  fi
  push+=(-- "$remote" "$head:refs/green/main")
  if $clear; then push+=(:refs/green/frozen); fi
  pushed=true
  push_output=$(git push "${push[@]}") || pushed=false
  printf '%s\n' "$push_output"
  # Git accepts an up-to-date ref even when its lease no longer matches.
  if ! $pushed || [[ "$push_output" == =* || "$push_output" == *$'\n='* ]]; then
    read_refs
    if [ -n "$green" ]; then
      ensure_commit "$green" || fail 'green moved during the check — nothing changed; run it again'
      if git merge-base --is-ancestor "$head" "$green"; then
        echo "covered: green is $green"
        exit 0
      fi
    fi
    fail 'green moved during the check — nothing changed; run it again'
  fi
  echo "green $head"
  if $clear; then echo "releases unfrozen (freeze at $frozen cleared)"; fi
  exit 0
fi
if [ -n "$green" ]; then
  suspects=$(git rev-list --first-parent --reverse "$green..$head")
else
  suspects=$(git rev-list --first-parent --reverse "$head")
fi
count=0; first=
while IFS= read -r oid; do
  [ -n "$oid" ] || continue
  if [ -z "$first" ]; then first=$oid; fi
  count=$((count + 1))
done <<< "$suspects"
if [ "$count" = 1 ]; then
  action="verify-main: releases are frozen. Revert or fix ${first:0:12}, then run this again."
  red="red $head culprit $first"
  code=5
else
  action="verify-main: releases are frozen. One of the $count commits from ${first:0:12} to ${head:0:12} broke main; revert or fix it, then run this again."
  red="red $head across $count commits from $first to $head"
  code=6
fi
still_on_main
if ! git push --porcelain "--force-with-lease=refs/green/frozen:$frozen" -- "$remote" "$head:refs/green/frozen"; then
  echo "$red"
  fail "${head:0:12} is red, and the freeze could not be pushed"
fi
echo "$action"
echo "$red"
exit "$code"
