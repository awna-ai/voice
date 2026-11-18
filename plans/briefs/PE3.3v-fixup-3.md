# PE3.3v fix-up 3: the Verifier's must-fix and should-fixes

Card `GhAdPMig`. Follows `plans/briefs/PE3.3v-verify-main.md` and fix-ups 1 and 2. Codex implements; V1
reviews and lands. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

Test first for each item: add the failing case to `tests/verify_main_test.sh`, run it and see it fail on
its own assertion, then change `scripts/verify-main.sh`. Keep every existing case passing. Only these two
files change (plus the stub `check.sh` the test writes). No new dependencies; Bash 3.2 compatible
(`/bin/bash` on macOS): no `mapfile`, no `${var,,}`, no associative arrays, no `wait -n`.

## 1. Must-fix: `main` is re-read just before either push

Today `head` is read once, before a check that can take hours, and then green or the freeze is pushed
unchecked. If `main` is rewound (force-pushed back) during the check, green lands on a commit that is no
longer on `main`.

- Add a function `still_on_main` and call it right before the green push (before building `push`) and
  right before the freeze push:
  ```bash
  still_on_main() {
    git fetch --quiet -- "$remote" main || fail "could not fetch $remote"
    git merge-base --is-ancestor "$head" "refs/remotes/$remote/main" ||
      fail "$remote/main no longer contains ${head:0:12} — nothing changed; run it again"
  }
  ```
  `head` being an ancestor (or equal) is fine: lands on top of it during the check are normal.
- On the red path, a `main` that no longer holds `head` also exits 1 with that line and pushes no freeze;
  print nothing about a culprit.
- Test (`rewind`): the stub, when `$VERIFY_TEST_TMP/rewind` exists, runs
  `git -C "$VERIFY_TEST_TMP/remote.git" update-ref refs/heads/main "$(cat "$VERIFY_TEST_TMP/rewind")"`.
  Case A: green is `G`, push a new commit `H`, write `G` to `rewind`, verify: exit 1, last line
  `verify-main: origin/main no longer contains <H12> — nothing changed; run it again`,
  `refs/green/main` still `G`. Case B: same with `RED` committed in `H`: exit 1, same line,
  `refs/green/frozen` unset. After each case: remove `rewind`, reset the clone with
  `git fetch -q origin && git reset -q --hard origin/main`, and leave the remote as the following cases
  expect (`main` back at `G`, `RED` absent).

## 2. The literal-argv case must be able to fail

The `PWNED` case writes `touch PWNED` relative to the check's working directory, the scratch worktree,
which is deleted, so the `find` can never see it.

- The stub writes its arguments, one per line, to `$VERIFY_TEST_TMP/args` (`printf '%s\n' "$@" >`,
  overwriting each run).
- The case passes `'a b;$(touch "$VERIFY_TEST_TMP/PWNED")'` as the argument (single-quoted in the test,
  keep the `shellcheck disable=SC2016`), then requires `$VERIFY_TEST_TMP/args` to hold exactly that
  string on one line, and `[ ! -e "$VERIFY_TEST_TMP/PWNED" ]`.

## 3. A live pid that isn't a verify-main run is stale

A pid in the lock can be reused by an unrelated process after a crash; today that blocks verify forever.

- A holder counts as running only when `kill -0 "$pid"` succeeds **and**
  `ps -p "$pid" -o command= | grep -q 'verify-main\.sh'`. Otherwise it is stale and handled as today
  (re-read, `rm -f` only if unchanged, one more `ln -s`).
- The test's live holder changes: write `$tmp/holder/verify-main.sh` containing
  `while :; do sleep 1; done`, start it with `bash "$tmp/holder/verify-main.sh" &`, and use `$!`. Kill
  it with `kill "$live"`, then `wait`.
- New case: `sleep 30 &` as the lock's pid (alive, not verify-main): the run treats it as stale, goes
  green, and no lock remains. Kill that `sleep` afterwards (extend the test's `cleanup` so every
  background pid it starts is killed on exit).

## 4. Lock ownership is tested both ways

Two mutants survive today: an unconditional `rm -f "$lock"` in `cleanup`, and dropping the
"remove only if unchanged" recheck before the stale removal.

- Case `steal` (covers cleanup): start a live holder (as in 3). The stub, when `$VERIFY_TEST_TMP/steal`
  exists, runs `ln -sfn "$(cat "$VERIFY_TEST_TMP/steal")" "$(git rev-parse --path-format=absolute
  --git-common-dir)/verify-main.lock"` (the stub runs in the scratch worktree, whose common dir is the
  clone's). Write the holder's pid to `steal`, verify: the run finishes (green), and afterwards the lock
  is still a symlink to the holder's pid. Then remove `steal`, the lock and the holder.
- Case `race` (covers the recheck): put a `readlink` shim first on `PATH` for this one run
  (`PATH="$tmp/shim:$PATH"`; pass it through `run` with `env`). The shim calls `/usr/bin/readlink "$@"`;
  on its first call only (a marker file in `$tmp`) it then runs `ln -sfn <live holder pid> <lock>` and
  prints the value it read before the swap. Seed the lock as a symlink to a dead pid (start and `wait` a
  `true` to get one). Expected: exit 0, last line `verify-main: already running (pid <holder>)`, zero
  check runs, the lock still points at the holder. With the recheck removed, the run would take the lock
  and go green: confirm that by hand once (a `cp` backup of the script, `cmp` to restore), and say so in
  your summary.

## 5. The freeze cases still missing

- **A freeze `head` doesn't contain.** Set `refs/green/frozen` in the bare remote to an orphan commit
  (fetch it in as case 13 does). A green run pushes green, does **not** clear the freeze, prints
  `green <head>` as its last line and no "unfrozen" line; `refs/green/frozen` is unchanged. Then delete
  that ref in the remote (`update-ref -d`) so later cases start clean.
- **The freeze push keeps its lease.** The stub, when `$VERIFY_TEST_TMP/movefreeze` exists, sets
  `refs/green/frozen` in the remote to its content. With `RED` on a new commit and `movefreeze` set to
  green's oid: exit 1, last line `verify-main: <head12> is red, and the freeze could not be pushed`,
  `refs/green/frozen` is still green's oid. Clean up the ref and `movefreeze` after.
- **A merge counts once.** From green, make a side branch with two commits, the second adding `RED`;
  back on `main`, `git merge -q --no-ff side -m merge`, push. Verify: exit 5,
  `red <merge> culprit <merge>`. Then run the existing `fix` helper.

## 6. Green current, freeze still set

When `green` equals `head` and a freeze is set at a commit `head` contains, the run today says
"green is current" and leaves releases frozen until something new lands.

- On that path: if `frozen` is set, `ensure_commit` it; if it is an ancestor of `head`, push
  `git push --porcelain "--force-with-lease=refs/green/frozen:$frozen" -- "$remote" :refs/green/frozen`
  (fail on error: `could not clear the freeze at <frozen12>`), print `green is current: <head>` then
  `releases unfrozen (freeze at <frozen> cleared)`, exit 0. A freeze `head` doesn't contain stays, and
  only the current line prints.
- Test: with green current, set `refs/green/frozen` to green's parent; verify: zero check runs, last line
  `releases unfrozen (freeze at <parent> cleared)`, the ref is gone.

## 7. A killed run's scratch worktree is cleaned up by the next

A SIGKILL skips the `EXIT` trap, so its scratch worktree stays registered and on disk.

- Create the scratch with `mktemp -d "${TMPDIR:-/tmp}/verify-main.XXXXXX"`.
- After the lock is taken (so no other run is live) and before the fetch: read
  `git worktree list --porcelain`; for each `worktree <path>` line whose path matches
  `*/verify-main.??????/wt`, run `git worktree remove --force -- "$path"` (ignore failure) and then
  `rmdir -- "${path%/wt}"` (ignore failure; never `rm -rf` a path read from git). Then `git worktree prune`.
- Test: `d=$(mktemp -d "${TMPDIR:-/tmp}/verify-main.XXXXXX")`, `git worktree add -q --detach "$d/wt"
  HEAD`, plus a decoy `git worktree add -q --detach "$tmp/decoy" HEAD`. Run verify; afterwards `$d` is
  gone and the decoy is still registered and on disk. Then remove the decoy, so `no_scratch` holds for
  the cases after it.

## Known limit (write it as a comment above `still_on_main`, one line)

A rewind in the seconds between the re-fetch and the push is not caught; the lease guards only the green
refs. That window is accepted.

## How to check

- Every new case red first, then green: `bash tests/verify_main_test.sh`, also with `/bin/bash`.
- `bash tests/ci_local_test.sh` green; `shellcheck scripts/verify-main.sh scripts/release-check.sh
  tests/verify_main_test.sh` clean.
- You can't commit in this sandbox: leave the change uncommitted and write the message to
  `/tmp/PE3.3v-fixup3-commit.txt`: `gate: main is re-read before green or a freeze is pushed, a reused pid
  or a killed run's scratch never blocks the next, and the lock and freeze cases are tested (PE3.3v
  fix-up 3, lane: V1)`. No AI attribution or co-author trailers.
