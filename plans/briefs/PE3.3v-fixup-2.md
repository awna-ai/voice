# PE3.3v fix-up 2: the security review's three low findings

Card `GhAdPMig`. Follows `plans/briefs/PE3.3v-verify-main.md` and fix-up 1. Codex implements; V1 reviews
and lands. Test first for each: add the failing case to `tests/verify_main_test.sh`, show it red, then fix.

## 1. A remote name may not start with `-`

`scripts/verify-main.sh` and `scripts/release-check.sh` reject a `--remote` value starting `--` but take
one starting with a single `-`. A remote can be named `-oevil` (`git remote add -- -oevil <url>`), and
`git push … "$remote"` would then read it as an option.

- Both scripts: `--remote` with a value starting `-` is a usage error (exit 2, the usage line), like `--`.
- Belt and braces: every `git push` in `verify-main.sh` puts `--` before the remote
  (`git push --atomic --porcelain <leases…> -- "$remote" <refspecs…>`; the options, leases included,
  all go before `--`). `git fetch` and `git ls-remote` already have it.
- Test: `--remote -oevil` exits 2 with the usage line for both scripts (add a remote by that name in the
  test's clone first, so the refusal is the script's, not git's "no such remote").

## 2. The lock is never taken by two runs, and never released by the wrong one

Between `mkdir "$lock"` and writing `$lock/pid`, a second run reads no pid, calls the lock stale and
removes it.

- The lock becomes a symlink whose target is the holder's pid, so the pid exists the moment the lock
  does: `ln -s "$$" "$lock"` is atomic and fails when `$lock` exists. Read the holder with
  `readlink "$lock"`. Stale (the target is not digits, or not a live pid): remove it only if `readlink`
  still gives the same stale value just before `rm -f "$lock"`, then try `ln -s` once more; losing that
  race prints the already-running line and exits 0. The lock path keeps its name,
  `$(git rev-parse --git-common-dir)/verify-main.lock`.
- Cleanup removes the lock only when `readlink "$lock"` is `$$`.
- Update the test's lock cases to the symlink form (a live pid: `ln -s <live pid> <lock>`; a dead pid
  likewise), and add: after a run, no `verify-main.lock` remains; a lock naming another live pid is
  still there after an "already running" exit.

## 3. `release-check.sh` checks green is on main

`release-check.sh` trusts `refs/green/main` as it reads it. If it pointed off `main`, it would approve
that commit's ancestors.

- After the frozen and no-green checks: `git fetch --quiet -- "$remote" main` (failure: exit 1,
  `release-check: could not fetch <remote>`), then require
  `git merge-base --is-ancestor "$green" "refs/remotes/$remote/main"`, else exit 1 with
  `release-check: green <green12> is not on <remote>/main; nothing can be released until verify-main.sh
  moves it.` Drop the old "fetch only if missing" branch; it's covered.
- Test: set `refs/green/main` in the bare remote to an orphan commit (as case 13 does), fetch it into the
  clone so it exists locally, and `release-check.sh` on it exits 1 with that sentence.

## How to check

- Each new case red first, then green: `bash tests/verify_main_test.sh`.
- `bash tests/ci_local_test.sh` green; `shellcheck scripts/verify-main.sh scripts/release-check.sh
  tests/verify_main_test.sh` clean. Must still run on `/bin/bash` 3.2.
- You can't commit in this sandbox: leave the change uncommitted and write the message to
  `/tmp/PE3.3v-fixup2-commit.txt`: `gate: a remote can't pass as an option, one lock holder at a time,
  and a release needs green on main (PE3.3v fix-up 2, lane: V1)`. No AI attribution or co-author
  trailers.
