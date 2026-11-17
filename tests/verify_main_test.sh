#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
live=
cleanup() {
  if [ -n "$live" ]; then kill "$live" 2>/dev/null || true; wait "$live" 2>/dev/null || true; fi
  rm -rf "$tmp"
}
trap cleanup EXIT
fail() { echo "verify_main_test: $*" >&2; exit 1; }
for script in verify-main release-check; do
  source_script=$repo/scripts/$script.sh
  [ -f "$source_script" ] || fail "scripts/$script.sh is missing"
done
git init --bare -q -b main "$tmp/remote.git"
git -c core.hooksPath=/dev/null clone -q "$tmp/remote.git" "$tmp/work" 2> "$tmp/clone.err"
cd "$tmp/work"
git config core.hooksPath /dev/null
git config user.name t
git config user.email t@example.invalid
git config commit.gpgsign false
mkdir scripts
cp "$repo/scripts/verify-main.sh" "$repo/scripts/release-check.sh" scripts/
export VERIFY_TEST_TMP="$tmp"
cat > "$tmp/check.sh" <<'CHECK'
#!/bin/bash
set -euo pipefail
printf '%s %s\n' "$(git rev-parse HEAD)" "$(pwd -P)" >> "$VERIFY_TEST_TMP/ran"
if [ -f "$VERIFY_TEST_TMP/move" ]; then
  git -C "$VERIFY_TEST_TMP/remote.git" update-ref refs/green/main "$(cat "$VERIFY_TEST_TMP/move")"
fi
if [ -f RED ]; then exit 1; fi
if [ -f "$VERIFY_TEST_TMP/flaky" ]; then rm "$VERIFY_TEST_TMP/flaky"; exit 1; fi
CHECK
chmod +x "$tmp/check.sh"
: > "$tmp/ran"
remote_ref() { git -C "$tmp/remote.git" rev-parse -q --verify "$1" || true; }
commit() { git add -A; git commit -q --allow-empty -m "$1"; }
push_commit() {
  commit "$1"
  git push -q origin HEAD:main
  head=$(git rev-parse HEAD)
}
run() {
  status=0
  "${test_bash:-bash}" "scripts/$1.sh" "${@:2}" > "$tmp/out" 2> "$tmp/err" || status=$?
}
expect() {
  [ "$status" = "$1" ] || { cat "$tmp/out" "$tmp/err" >&2; fail "expected exit $1, got $status"; }
  [ "$(tail -n 1 "$tmp/out")" = "$2" ] || { cat "$tmp/out" >&2; fail "expected last line: $2"; }
}
ref_is() { [ "$(remote_ref "$1")" = "$2" ] || fail "unexpected $1"; }
runs() { [ "$(wc -l < "$tmp/ran" | tr -d ' ')" = "$1" ] || fail "expected $1 check runs"; }
verify() { : > "$tmp/ran"; run verify-main --remote origin -- "$tmp/check.sh"; }
no_scratch() {
  [ "$(git worktree list --porcelain | grep -c '^worktree ')" = 1 ] || fail 'scratch worktree remains'
  local lock
  lock="$(git rev-parse --git-common-dir)/verify-main.lock"
  [ ! -e "$lock" ] && [ ! -L "$lock" ] || fail 'lock remains'
}
push_commit initial
usage='Usage: scripts/verify-main.sh [--remote <name>] [-- <check command>...]'
for arg in --bogus --remote; do
  run verify-main "$arg"
  [ "$status" = 2 ] && [ "$(cat "$tmp/err")" = "$usage" ] || fail 'verify usage'
done
for arg in nosuch --bogus; do
  run verify-main --remote "$arg"
  [ "$status" = 2 ] && [ "$(cat "$tmp/err")" = "$usage" ] || fail 'remote usage'
done
git remote add -- -oevil "$tmp/remote.git"
run verify-main --remote -oevil -- "$tmp/check.sh"
[ "$status" = 2 ] && [ "$(cat "$tmp/err")" = "$usage" ] || fail 'verify single-dash remote usage'
echo 'verify_main_test: verify single-dash remote passed'
run verify-main --
[ "$status" = 2 ] && [ "$(cat "$tmp/err")" = "$usage" ] || fail 'empty check usage'
run release-check
[ "$status" = 2 ] && [ "$(cat "$tmp/err")" = 'Usage: scripts/release-check.sh [--remote <name>] <commit>' ] || fail 'release usage'
run release-check --remote -oevil main
[ "$status" = 2 ] && [ "$(cat "$tmp/err")" = 'Usage: scripts/release-check.sh [--remote <name>] <commit>' ] || fail 'release single-dash remote usage'
echo 'verify_main_test: release single-dash remote passed'
run release-check nosuchref
[ "$status" = 2 ] && [ "$(cat "$tmp/err")" = 'release-check: nosuchref is not a commit' ] || fail 'unknown commit'
echo 'verify_main_test: usage passed'
verify
expect 0 "green $head"
ref_is refs/green/main "$head"
ref_is refs/green/frozen ''
runs 1
read -r checked directory < "$tmp/ran"
[ "$checked" = "$head" ] && [ "$directory" != "$(pwd -P)" ] || fail 'check used caller checkout'
no_scratch
run verify-main -- "$tmp/check.sh"
expect 0 "green is current: $head"
runs 1
echo 'verify_main_test: first green, scratch cleanup and current green passed'
batch() {
  push_commit batch-one
  push_commit batch-two
  verify
  expect 0 "green $head"
  ref_is refs/green/main "$head"
  runs 1
}
batch
old=$head
push_commit remote-newer
git checkout -q --detach "$old"
printf 'keep\n' > uncommitted
verify
expect 0 "green $head"
runs 1
read -r checked directory < "$tmp/ran"
[ "$checked" = "$head" ] && [ -f uncommitted ] && [ "$(git rev-parse HEAD)" = "$old" ] || fail 'caller checkout changed'
rm uncommitted
git checkout -q main
: > "$tmp/flaky"
push_commit flaky
verify
expect 0 "green $head"
runs 2
grep -Fx "verify-main: ${head:0:12} passed on the rerun (the first run was red)" "$tmp/out" >/dev/null || fail 'missing rerun sentence'
green=$head
: > RED
push_commit red
red=$head
verify
expect 5 "red $head culprit $head"
runs 2
ref_is refs/green/main "$green"
ref_is refs/green/frozen "$red"
[ "$(tail -n 2 "$tmp/out" | head -n 1)" = "verify-main: releases are frozen. Revert or fix ${red:0:12}, then run this again." ] || fail 'missing fix sentence'
no_scratch
run release-check "$green"
expect 1 "release-check: releases are frozen at ${red:0:12}: main went red twice there. Revert or fix it, and wait for the next green."
fix() {
  rm RED
  push_commit fix
  verify
  expect 0 "releases unfrozen (freeze at $red cleared)"
  ref_is refs/green/main "$head"
  ref_is refs/green/frozen ''
}
fix
: > RED
push_commit first-red
first=$head
push_commit second-red
red=$head
verify
expect 6 "red $head across 2 commits from $first to $head"
ref_is refs/green/frozen "$head"
runs 2
fix
echo 'verify_main_test: batch, caller isolation, rerun, red and unfreeze passed'
run release-check main
expect 0 "release-check: ${head:0:12} passed the full check (green is ${head:0:12})."
green=$head
git tag checked "$green"
run release-check checked
expect 0 "release-check: ${green:0:12} passed the full check (green is ${green:0:12})."
push_commit not-checked
run release-check main
expect 1 "release-check: ${head:0:12} has not passed the full check on main yet (green is ${green:0:12}). Release a green commit, or run scripts/verify-main.sh first."
git init --bare -q -b main "$tmp/empty.git"
git remote add empty "$tmp/empty.git"
run release-check --remote empty main
expect 1 'release-check: nothing on main has passed the full check yet.'
orphan=$(printf 'orphan\n' | git commit-tree "HEAD^{tree}")
git -C "$tmp/remote.git" fetch -q "$tmp/work" "$orphan"
git -C "$tmp/remote.git" update-ref refs/green/main "$orphan"
git fetch -q origin refs/green/main
run release-check "$orphan"
expect 1 "release-check: green ${orphan:0:12} is not on origin/main; nothing can be released until verify-main.sh moves it."
echo 'verify_main_test: release off-main green passed'
verify
expect 1 "verify-main: green ${orphan:0:12} is not on origin/main — nothing changed"
runs 0
ref_is refs/green/main "$orphan"
git -C "$tmp/remote.git" update-ref refs/green/main "$green"
printf '%s\n' "$head" > "$tmp/move"
verify
expect 0 "covered: green is $head"
ref_is refs/green/main "$head"
push_commit lease-conflict
printf '%s\n' "$old" > "$tmp/move"
verify
expect 1 'verify-main: green moved during the check — nothing changed; run it again'
ref_is refs/green/main "$old"
rm "$tmp/move"
echo 'verify_main_test: release checks, off-main green and leases passed'
lock="$(git rev-parse --git-common-dir)/verify-main.lock"
sleep 30 &
live=$!
ln -s "$live" "$lock"
verify
expect 0 "verify-main: already running (pid $live)"
runs 0
[ -L "$lock" ] && [ "$(readlink "$lock")" = "$live" ] || fail 'live lock changed'
kill "$live"
wait "$live" 2>/dev/null || true
live=
verify
expect 0 "green $head"
runs 1
[ ! -e "$lock" ] && [ ! -L "$lock" ] || fail 'lock remains'
push_commit strange-argument
: > "$tmp/ran"
# shellcheck disable=SC2016
run verify-main -- "$tmp/check.sh" 'a b;$(touch PWNED)'
expect 0 "green $head"
[ -z "$(find "$tmp" -name PWNED -print)" ] || fail 'check argument was evaluated'
if [ "$(/bin/bash -c 'echo "${BASH_VERSINFO[0]}"')" = 3 ]; then
  test_bash=/bin/bash
  batch
  echo 'verify_main_test: Bash 3.2 batch passed'
fi
printf '%s\n' refs/green/main refs/heads/main > "$tmp/refs"
git -C "$tmp/remote.git" for-each-ref --format='%(refname)' > "$tmp/actual-refs"
cmp "$tmp/refs" "$tmp/actual-refs" || fail 'unexpected remote ref'
no_scratch
echo 'verify_main_test: locking, literal argv and remote ref limits passed'
echo 'verify_main_test: all passed'
