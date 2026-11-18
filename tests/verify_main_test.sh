#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
live=
unrelated=
d=
cleanup() {
  if [ -n "$live" ]; then kill "$live" 2>/dev/null || true; wait "$live" 2>/dev/null || true; fi
  if [ -n "$unrelated" ]; then kill "$unrelated" 2>/dev/null || true; wait "$unrelated" 2>/dev/null || true; fi
  if [ -n "$d" ]; then
    git -C "$tmp/work" worktree remove --force -- "$d/wt" >/dev/null 2>&1 || true
    rmdir -- "$d" 2>/dev/null || true
  fi
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
printf '%s\n' "$@" > "$VERIFY_TEST_TMP/args"
printf '%s %s\n' "$(git rev-parse HEAD)" "$(pwd -P)" >> "$VERIFY_TEST_TMP/ran"
if [ -f "$VERIFY_TEST_TMP/move" ]; then
  git -C "$VERIFY_TEST_TMP/remote.git" update-ref refs/green/main "$(cat "$VERIFY_TEST_TMP/move")"
fi
if [ -f "$VERIFY_TEST_TMP/rewind" ]; then
  git -C "$VERIFY_TEST_TMP/remote.git" update-ref refs/heads/main "$(cat "$VERIFY_TEST_TMP/rewind")"
fi
if [ -f "$VERIFY_TEST_TMP/steal" ]; then
  ln -sfn "$(cat "$VERIFY_TEST_TMP/steal")" "$(git rev-parse --path-format=absolute --git-common-dir)/verify-main.lock"
fi
if [ -f "$VERIFY_TEST_TMP/movefreeze" ]; then
  git -C "$VERIFY_TEST_TMP/remote.git" update-ref refs/green/frozen "$(cat "$VERIFY_TEST_TMP/movefreeze")"
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
  local runner
  runner=("${test_bash:-bash}")
  if [ "$1" = env ]; then runner=(env "$2" "${test_bash:-bash}"); shift 2; fi
  status=0
  "${runner[@]}" "scripts/$1.sh" "${@:2}" > "$tmp/out" 2> "$tmp/err" || status=$?
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
# A tag named main must not hide a newer branch head from either script.
tagged_old=$head
git push -q origin "$head:refs/tags/main"
commit tagged-main
git push -q origin HEAD:refs/heads/main
head=$(git rev-parse HEAD)
git update-ref refs/remotes/origin/main "$tagged_old"
verify
expect 0 "green $head"
ref_is refs/green/main "$head"
runs 1
run release-check "$head"
expect 0 "release-check: ${head:0:12} passed the full check (green is ${head:0:12})."
git -C "$tmp/remote.git" tag -d main >/dev/null
if git show-ref --verify --quiet refs/tags/main; then git tag -d main >/dev/null; fi
no_scratch
echo 'verify_main_test: tag named main passed'
# A killed run leaves its scratch registered; an unrelated worktree must stay.
d=$(mktemp -d "${TMPDIR:-/tmp}/verify-main.XXXXXX")
git worktree add -q --detach "$d/wt" HEAD
git worktree add -q --detach "$tmp/decoy" HEAD
verify
expect 0 "green is current: $head"
[ ! -d "$d" ] || fail 'killed run scratch remains'
[ -d "$tmp/decoy" ] || fail 'decoy worktree removed'
decoy=$(cd "$tmp/decoy" && pwd -P)
git worktree list --porcelain | grep -Fx "worktree $decoy" >/dev/null || fail 'decoy worktree unregistered'
git worktree remove -- "$tmp/decoy"
d=
no_scratch
echo 'verify_main_test: killed run scratch swept and decoy preserved passed'
# A scratch parent replaced by a symlink must not remove its target worktree.
git worktree add -q --detach "$tmp/target/wt" HEAD
printf 'keep\n' > "$tmp/target/wt/untracked"
real=$(mktemp -d "$tmp/real.XXXXXX")
d=$real/verify-main.abcdef
mkdir "$d"
git worktree add -q --detach "$d/wt" HEAD
rm -rf "$d"
ln -s "$tmp/target" "$d"
verify
expect 0 "green is current: $head"
[ -d "$tmp/target/wt" ] && [ -f "$tmp/target/wt/untracked" ] || fail 'symlink target worktree removed'
rm "$d"
d=
git worktree remove --force -- "$tmp/target/wt"
git worktree prune
no_scratch
echo 'verify_main_test: symlink target worktree preserved passed'
# A check may finish after main has been rewound, on either outcome.
G=$head
for outcome in green red; do
  if [ "$outcome" = red ]; then : > RED; fi
  push_commit rewind
  printf '%s\n' "$G" > "$tmp/rewind"
  verify
  expect 1 "verify-main: origin/main no longer contains ${head:0:12} — nothing changed; run it again"
  ref_is refs/green/main "$G"
  ref_is refs/green/frozen ''
  if grep -q culprit "$tmp/out"; then fail 'rewind named a culprit'; fi
  rm "$tmp/rewind"
  git fetch -q origin
  git reset -q --hard origin/main
  head=$G
  no_scratch
done
echo 'verify_main_test: green and red main rewinds passed'
batch() {
  push_commit batch-one
  push_commit batch-two
  verify
  expect 0 "green $head"
  ref_is refs/green/main "$head"
  runs 1
}
batch
parent=$(git rev-parse "$head^")
git -C "$tmp/remote.git" update-ref refs/green/frozen "$parent"
verify
expect 0 "releases unfrozen (freeze at $parent cleared)"
grep -Fx "green is current: $head" "$tmp/out" >/dev/null || fail 'missing current green line'
runs 0
ref_is refs/green/frozen ''
echo 'verify_main_test: current green clears ancestor freeze passed'
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
# A freeze on another history must survive a passing main.
orphan_freeze=$(printf 'orphan freeze\n' | git commit-tree "HEAD^{tree}")
git -C "$tmp/remote.git" fetch -q "$tmp/work" "$orphan_freeze"
git -C "$tmp/remote.git" update-ref refs/green/frozen "$orphan_freeze"
push_commit unrelated-freeze
verify
expect 0 "green $head"
ref_is refs/green/main "$head"
ref_is refs/green/frozen "$orphan_freeze"
if grep -q unfrozen "$tmp/out"; then fail 'unrelated freeze reported cleared'; fi
verify
expect 0 "green is current: $head"
grep -Fx "verify-main: releases are still frozen at ${orphan_freeze:0:12}, which is not on origin/main; check it and delete refs/green/frozen on origin by hand." "$tmp/out" >/dev/null || fail 'missing unrelated freeze sentence'
ref_is refs/green/frozen "$orphan_freeze"
runs 0
git -C "$tmp/remote.git" update-ref -d refs/green/frozen
echo 'verify_main_test: unrelated freeze preserved passed'
# A competing freeze update must win over this run's stale lease.
green=$head
: > RED
push_commit freeze-lease
printf '%s\n' "$green" > "$tmp/movefreeze"
verify
expect 1 "verify-main: ${head:0:12} is red, and the freeze could not be pushed"
runs 2
ref_is refs/green/main "$green"
ref_is refs/green/frozen "$green"
git -C "$tmp/remote.git" update-ref -d refs/green/frozen
rm "$tmp/movefreeze" RED
push_commit freeze-lease-fixed
verify
expect 0 "green $head"
echo 'verify_main_test: freeze push lease passed'
# Only the first-parent merge is a suspect, not its side commits.
git checkout -q -b side
commit side-one
: > RED
commit side-red
git checkout -q main
git merge -q --no-ff side -m merge
git push -q origin HEAD:main
head=$(git rev-parse HEAD)
red=$head
verify
expect 5 "red $head culprit $head"
runs 2
ref_is refs/green/frozen "$head"
fix
echo 'verify_main_test: merge counts once passed'
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
mkdir "$tmp/holder"
printf '%s\n' 'while :; do sleep 1; done' > "$tmp/holder/verify-main.sh"
bash "$tmp/holder/verify-main.sh" &
live=$!
ln -s "$live" "$lock"
verify
expect 0 "verify-main: already running (pid $live)"
runs 0
[ -L "$lock" ] && [ "$(readlink "$lock")" = "$live" ] || fail 'live lock changed'
: > "$tmp/ran"
run env COLUMNS=10 verify-main -- "$tmp/check.sh"
expect 0 "verify-main: already running (pid $live)"
runs 0
[ -L "$lock" ] && [ "$(readlink "$lock")" = "$live" ] || fail 'narrow-column live lock changed'
echo 'verify_main_test: narrow-column live holder passed'
kill "$live"
wait "$live" 2>/dev/null || true
live=
verify
expect 0 "green $head"
runs 1
[ ! -e "$lock" ] && [ ! -L "$lock" ] || fail 'lock remains'
push_commit reused-pid
sleep 30 &
unrelated=$!
ln -s "$unrelated" "$lock"
verify
expect 0 "green $head"
runs 1
no_scratch
kill "$unrelated"
wait "$unrelated" 2>/dev/null || true
unrelated=
echo 'verify_main_test: reused unrelated pid passed'
# Cleanup must not remove a lock another holder took during the check.
push_commit stolen-lock
bash "$tmp/holder/verify-main.sh" &
live=$!
printf '%s\n' "$live" > "$tmp/steal"
verify
expect 0 "green $head"
runs 1
[ -L "$lock" ] && [ "$(readlink "$lock")" = "$live" ] || fail 'stolen lock removed by cleanup'
rm "$tmp/steal" "$lock"
kill "$live"
wait "$live" 2>/dev/null || true
live=
echo 'verify_main_test: stolen lock preserved by cleanup passed'
# Swap a stale lock after its first read, before the unchanged-owner recheck.
push_commit raced-lock
bash "$tmp/holder/verify-main.sh" &
live=$!
true &
dead=$!
wait "$dead"
ln -s "$dead" "$lock"
mkdir "$tmp/shim"
printf '%s\n' "$live" > "$tmp/race-holder"
cat > "$tmp/shim/readlink" <<'READLINK'
#!/bin/bash
set -euo pipefail
value=$(/usr/bin/readlink "$@")
if [ ! -e "$VERIFY_TEST_TMP/readlink-swapped" ]; then
  : > "$VERIFY_TEST_TMP/readlink-swapped"
  ln -sfn "$(cat "$VERIFY_TEST_TMP/race-holder")" "$1"
fi
printf '%s\n' "$value"
READLINK
chmod +x "$tmp/shim/readlink"
: > "$tmp/ran"
run env "PATH=$tmp/shim:$PATH" verify-main -- "$tmp/check.sh"
expect 0 "verify-main: already running (pid $live)"
runs 0
[ -L "$lock" ] && [ "$(readlink "$lock")" = "$live" ] || fail 'raced lock changed'
rm "$lock"
kill "$live"
wait "$live" 2>/dev/null || true
live=
echo 'verify_main_test: stale lock ownership recheck passed'
push_commit strange-argument
: > "$tmp/ran"
# shellcheck disable=SC2016
run verify-main -- "$tmp/check.sh" 'a b;$(touch "$VERIFY_TEST_TMP/PWNED")'
expect 0 "green $head"
# shellcheck disable=SC2016
printf '%s\n' 'a b;$(touch "$VERIFY_TEST_TMP/PWNED")' > "$tmp/expected-args"
cmp "$tmp/expected-args" "$tmp/args" || fail 'check argument was changed'
[ ! -e "$tmp/PWNED" ] || fail 'check argument was evaluated'
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
