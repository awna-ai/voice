# PE3.3v fix-up 5: the repeat Verifier's must-fix and the security review's two findings

Card `GhAdPMig`. Follows fix-ups 3 and 4. Codex implements; V1 reviews and lands. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`.

Test first where a test is named: add the case to `tests/verify_main_test.sh`, see it fail on its own
assertion, then change the script. Keep every existing case green. Bash 3.2 compatible.

## 1. A tag named `main` can't freeze the tracking ref (security, medium)

`git fetch -- "$remote" main` resolves the short name `main` as `refs/tags/main` before `refs/heads/main`.
A remote tag `main` then goes to `FETCH_HEAD` only, `refs/remotes/<remote>/main` stays stale, and both the
rewind guard and `release-check.sh` compare against the stale ref.

- In `scripts/verify-main.sh` (the first fetch and `still_on_main`) and `scripts/release-check.sh`, fetch
  with an explicit refspec: `git fetch --quiet -- "$remote" "+refs/heads/main:refs/remotes/$remote/main"`.
- Test: push a tag `main` to the test's bare remote (`git push -q origin <some older commit>:refs/tags/main`),
  push a new commit to `main`, and verify: green moves to the new commit (it would stay at the old head
  without the fix). Delete the remote tag (`git -C "$tmp/remote.git" tag -d main`) and the clone's local tag
  if one was fetched, afterwards.

## 2. The holder's command is read in full (Verifier, must-fix)

procps-ng's `ps` (Linux) cuts `-o command=` to `$COLUMNS` even with no terminal, so a long path loses
`verify-main.sh` and a live run is read as stale; the second run's sweep then removes the first run's live
scratch worktree.

- Use `ps -ww -p "$pid" -o command=`.
- Drop `kill -0`: it fails with EPERM for another user's live process. A non-empty command matching
  `*verify-main.sh*` is enough. Keep the digits check first, and only call `ps` once the pid is all digits.
- A lock naming this run's own pid (`$$`, a dead run's lock that happens to share it) is stale.
- Test: run the live-holder case once more with `COLUMNS=10` exported for the run (`run env COLUMNS=10
  verify-main …`, as the `PATH` case passes its env): still `already running (pid <holder>)`. It can only
  fail on Linux; that's accepted. The `$$` guard gets no test (the script's pid can't be known before it
  starts).

## 3. The sweep follows no symlink (security, low)

`git worktree remove --force` resolves the path through `realpath`, so a `verify-main.XXXXXX` that was
deleted and replaced by a symlink could point the sweep at another worktree.

- In the sweep loop, before the remove: `parent=${path%/wt}` and
  `[ -d "$parent" ] && [ ! -L "$parent" ] && [ -O "$parent" ] || continue`. A skipped entry is left for
  the `git worktree prune` after the loop.
- Test: make `$tmp/target/wt` a worktree of the clone with an untracked file in it; make a directory path
  matching the pattern a symlink to `$tmp/target` (`ln -s "$tmp/target" "${TMPDIR:-/tmp}/verify-main.<6
  chars>"` with a name from `mktemp -u`), and register a worktree entry for `<symlink>/wt` is not possible
  directly, so instead: `git worktree add` at the real path `$tmp/real.XXXXXX/verify-main.abcdef/wt`
  (mkdir the parent), then `rm -rf` that `verify-main.abcdef` directory and replace it with a symlink to
  `$tmp/target`. Verify; afterwards `$tmp/target/wt` and its untracked file still exist. Remove the symlink,
  the target worktree and `git worktree prune` afterwards so `no_scratch` holds.

## 4. Two messages (Verifier, should-fix)

- Green current and a freeze `head` doesn't contain: print
  `verify-main: releases are still frozen at <frozen12>, which is not on <remote>/main; check it and delete refs/green/frozen on <remote> by hand.`
  before the `green is current: <head>` line (which stays the last line, exit 0). Test: the
  "unrelated freeze" case runs `verify` once more after its green (green is now current) and finds that
  sentence; the ref is unchanged.
- When the green push fails and `read_refs` shows `frozen` changed from what this run read, fail with
  `the freeze moved during the check — nothing changed; run it again` instead of the "green moved" line.
  No test needed.

## How to check

- `bash tests/verify_main_test.sh` green, also with `test_bash=/bin/bash` paths as today; it needs `ps`.
  If your sandbox denies `ps`, say which cases you could not run.
- `bash tests/ci_local_test.sh` green; `shellcheck scripts/verify-main.sh scripts/release-check.sh
  tests/verify_main_test.sh` clean.
- Leave the change uncommitted; write the message to `/tmp/PE3.3v-fixup5-commit.txt`: `gate: main is
  fetched by its full ref, the holder's command is read in full, and the sweep follows no symlink (PE3.3v
  fix-up 5, lane: V1)`. No AI attribution or co-author trailers.
