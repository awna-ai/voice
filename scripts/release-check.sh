#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
usage() {
  echo 'Usage: scripts/release-check.sh [--remote <name>] <commit>' >&2
  exit 2
}
remote=origin
commit=
while [ "$#" -gt 0 ]; do
  case "$1" in
    --remote)
      [ "$#" -ge 2 ] && [ -n "$2" ] && [[ "$2" != --* ]] || usage
      remote=$2; shift 2 ;;
    -*) usage ;;
    *) [ -z "$commit" ] && [ -n "$1" ] || usage; commit=$1; shift ;;
  esac
done
[ -n "$commit" ] || usage
git remote get-url -- "$remote" >/dev/null 2>&1 || usage
if ! candidate=$(git rev-parse --verify --quiet --end-of-options "$commit^{commit}"); then
  echo "release-check: $commit is not a commit" >&2
  exit 2
fi
fail() { echo "release-check: $*"; exit 1; }
refs=$(git ls-remote -- "$remote" refs/green/main refs/green/frozen) || fail "could not read $remote"
green=; frozen=
while read -r oid ref; do
  [ -n "$oid" ] || continue
  [[ "$oid" =~ ^[[:xdigit:]]{40}$ || "$oid" =~ ^[[:xdigit:]]{64}$ ]] || fail "invalid oid from $remote"
  case "$ref" in refs/green/main) green=$oid ;; refs/green/frozen) frozen=$oid ;; esac
done <<< "$refs"
[ -z "$frozen" ] || fail "releases are frozen at ${frozen:0:12}: main went red twice there. Revert or fix it, and wait for the next green."
[ -n "$green" ] || fail 'nothing on main has passed the full check yet.'
if ! git cat-file -e "$green^{commit}" 2>/dev/null; then
  git fetch --quiet -- "$remote" main || fail "could not fetch $remote"
  git cat-file -e "$green^{commit}" 2>/dev/null || fail "could not read green from $remote"
fi
if git merge-base --is-ancestor "$candidate" "$green"; then
  echo "release-check: ${candidate:0:12} passed the full check (green is ${green:0:12})."
else
  fail "${candidate:0:12} has not passed the full check on main yet (green is ${green:0:12}). Release a green commit, or run scripts/verify-main.sh first."
fi
