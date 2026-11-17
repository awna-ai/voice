# PE3.3v: voice's main gets a full check after lands, and a release reads it

Card `GhAdPMig`. Spec: lane V's spec file § PE3.3v, and the process-evaluation plan § PE3.3 (the plan wins).
Codex implements this brief; V1 (Claude) reviews, gates and lands it.

Read first, and follow: `AGENTS.md`, `.ai/PROJECT.md` (the path-to-test table), `CONTRIBUTING.md`,
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`.

## Why

A land runs the gate on its own branch. Nothing checks `main` itself after lands, and the lane's land gate
is moving to the touched tier (`scripts/ci-local.sh --tier touched`), which skips the parts a change
doesn't map to. So `main` needs a full-gate check of its own, run on a batch of lands at a time, and a
release must ship only a commit that passed it.

## Decisions already made (don't revisit)

- **The green ref lives on the remote, as `refs/green/main`.** It always points at a commit on `main`
  that passed the full gate. A second ref, `refs/green/frozen`, exists only while releases are frozen,
  and points at the `main` commit that went red twice. Both hold only commits that are already on `main`,
  so pushing them adds no history and no data to the public repo. Nothing else is pushed.
- **voice gets its own small script, not a borrowed tool.** The script is generic: it knows a remote, a
  check command and two refs, nothing about who schedules it. No scheduler, machine, job or tool name
  of ours appears in voice (the repo is public). A scheduler calls it later, from outside the repo.
- **One rerun.** A red check is run once more on the same commit. Green on the rerun counts as green
  (and says it was a rerun). Red twice freezes releases and names what to revert or fix.
- **No automatic revert.** The script names the commit; a person or a lane reverts or fixes it.

## What to build

1. `scripts/verify-main.sh` (new): verify the remote's `main` and move the green ref.
2. `scripts/release-check.sh` (new): refuse to release a commit that is not green.
3. `tests/verify_main_test.sh` (new), written first and shown red (the scripts don't exist yet).
4. `scripts/ci-local.sh`: run the new test in the full tier, shellcheck the three new files, and map
   them in the touched tier. `tests/ci_local_test.sh`: update its golden lists to match.
5. Docs: `.ai/PROJECT.md`'s table, and a short *Releasing* section in `CONTRIBUTING.md`.

Bash only, plus git. Must run on macOS's stock `/bin/bash` 3.2 (no associative arrays, `mapfile`,
`${x,,}`, `declare -n`; empty arrays under `set -u` need `${a[@]+"${a[@]}"}`). `set -euo pipefail`.
Both scripts find the repo from their own location (`cd "$(dirname "$0")/.."`); no fixed paths.

### `scripts/verify-main.sh`

```
bash scripts/verify-main.sh [--remote <name>] [-- <check command> [args...]]
```

- `--remote` defaults to `origin`; it must name a configured remote (`git remote get-url`), else usage.
- The check defaults to `bash scripts/ci-local.sh` (the full tier). Everything after `--` is the check,
  run as an argv (no `eval`, no `sh -c` of a joined string).
- Usage errors (unknown option, `--remote` with no value or a value starting `--`, `--` with nothing
  after it): print `Usage: scripts/verify-main.sh [--remote <name>] [-- <check command>...]` to stderr,
  exit 2.

Steps:

1. **One at a time.** Take a lock: `mkdir "$(git rev-parse --git-common-dir)/verify-main.lock"` and write
   `$$` to a `pid` file in it; release it on exit (trap). If the directory exists and its pid is alive
   (`kill -0`), print `verify-main: already running (pid N)` and exit 0 without running anything. If the
   pid is dead, unreadable or missing, take the lock over (remove and re-create; if that `mkdir` loses a
   race, print the already-running line and exit 0).
2. **Read the remote.** `git fetch --quiet <remote> main` (a failure: exit 1 with
   `verify-main: could not fetch <remote>`). `head` = `git rev-parse --verify refs/remotes/<remote>/main^{commit}`.
   Read `refs/green/main` and `refs/green/frozen` with one `git ls-remote <remote> refs/green/main
   refs/green/frozen`; validate each oid is 40 or 64 hex characters before using it. A failed
   `ls-remote` is exit 1, never "no green".
3. **Nothing new.** If green equals `head`: last line `green is current: <head>`, exit 0. The check does
   not run.
4. **Green must be on main.** If green is set and is not an ancestor of `head`
   (`git merge-base --is-ancestor`; fetch it first with `git fetch --quiet <remote> <oid>` only if
   `git cat-file -e <oid>^{commit}` fails): exit 1 with `verify-main: green <green12> is not on
   <remote>/main — nothing changed`. Never move green backwards or sideways.
5. **The check runs on `head`, never on the caller's checkout.** Create a scratch worktree:
   `git worktree add --quiet --detach <mktemp -d>/wt <head>`; remove it on every exit path
   (`git worktree remove --force`, then the temp dir), including red, a signal and a lease failure. Run
   the check with the scratch worktree as its working directory. Its output goes to the script's stdout
   and stderr as it runs. Before each run print `verify-main: checking <head12> (<n> commits since green
   <green12>)` (or `(no green yet)`).
6. **Pass** (exit 0, on the first run or the rerun): advance green. On a rerun pass, print first
   `verify-main: <head12> passed on the rerun (the first run was red)`.
7. **Red** (non-zero): print `verify-main: <head12> was red (exit <n>); running it once more`, and run
   the check again in the same worktree. Red again: freeze (below).

**Advancing green.** One atomic push, so the green ref and the freeze never disagree:

```
git push --atomic --porcelain <remote> \
  --force-with-lease=refs/green/main:<old green or empty> <head>:refs/green/main \
  [--force-with-lease=refs/green/frozen:<frozen> :refs/green/frozen]
```

- Delete the freeze in the same push only when the frozen commit is an ancestor of, or equal to, `head`
  (the red commit is now inside a passing `main`). A freeze `head` does not contain stays.
- An empty lease value (`refs/green/main:`) means "must not exist yet", for the first green.
- The last line is `green <head>`, or, when the freeze was deleted,
  `releases unfrozen (freeze at <frozen> cleared)` after a `green <head>` line. Exit 0.
- **The lease fails** (someone moved green or the freeze during the check): re-read both refs. If the
  new green equals `head` or descends from it, last line `covered: green is <new green>`, exit 0.
  Otherwise exit 1 with `verify-main: green moved during the check — nothing changed; run it again`.
  Never retry the push blindly.
- Never pass `--no-verify`. A pre-push hook in the caller's checkout runs as for any push.

**Freezing (red twice).**

- The commits under suspicion are `git rev-list --first-parent --reverse <green>..<head>`, or, with no
  green yet, every first-parent commit of `head`.
- Push the freeze: `git push --porcelain <remote>
  --force-with-lease=refs/green/frozen:<current frozen or empty> <head>:refs/green/frozen`. If releases
  were already frozen at an older commit, still move the freeze to `head` (the newest red is the one to
  fix). A failed freeze push: exit 1 with `verify-main: <head12> is red, and the freeze could not be
  pushed`, after printing the red line below on the line before it, so a reader still sees which commit.
- Exactly one commit under suspicion: last line `red <head> culprit <commit>`, exit 5.
- More than one: last line `red <head> across <n> commits from <first> to <head>`, exit 6.
- Before that last line, print one plain line a person can act on:
  `verify-main: releases are frozen. Revert or fix <commit12>, then run this again.` (one commit) or
  `verify-main: releases are frozen. One of the <n> commits from <first12> to <head12> broke main;
  revert or fix it, then run this again.`
- Green does not move on red.

Exit codes, all in one comment block at the top: 0 green, current, covered or already running; 1 failed
(nothing judged); 2 usage; 5 red with one culprit; 6 red across several commits. Full 40/64-character
oids on the last line; 12-character short forms only in prose lines.

### `scripts/release-check.sh`

```
bash scripts/release-check.sh [--remote <name>] <commit>
```

- `--remote` as above, default `origin`. `<commit>` is required (a tag, a branch or an oid); resolve it
  with `git rev-parse --verify --quiet "<commit>^{commit}"` (unknown: exit 2 with
  `release-check: <commit> is not a commit`). Usage errors exit 2 with
  `Usage: scripts/release-check.sh [--remote <name>] <commit>`.
- Read `refs/green/main` and `refs/green/frozen` with one `git ls-remote` (failure: exit 1,
  `release-check: could not read <remote>` — never "green"). Validate the oids as above. Fetch green
  only if it is not present locally (`git fetch --quiet <remote> main`; if still missing, exit 1).
- Frozen: exit 1, `release-check: releases are frozen at <frozen12>: main went red twice there. Revert
  or fix it, and wait for the next green.` This holds for every commit, green or not.
- No green yet: exit 1, `release-check: nothing on main has passed the full check yet.`
- `<commit>` is green or an ancestor of green: exit 0, `release-check: <commit12> passed the full check
  (green is <green12>).`
- Otherwise: exit 1, `release-check: <commit12> has not passed the full check on main yet (green is
  <green12>). Release a green commit, or run scripts/verify-main.sh first.`
- It reads only. It never pushes, fetches anything but `main` or green, or changes the checkout.

### `tests/verify_main_test.sh`

Bash, `set -euo pipefail`, finds the repo from its own location, cleans up with a `trap`, no network.
Write it first and show it red (the scripts are missing).

Setup in a `mktemp -d`:

- `git init --bare -b main remote.git`; a clone `work` of it with `-c core.hooksPath=/dev/null`
  commits (`user.name=t`, `user.email=t@example.invalid`, `commit.gpgsign=false`); copy the caller's
  `scripts/verify-main.sh` and `scripts/release-check.sh` into `work/scripts/`; commit and push `main`.
  The scripts run from `work` with `--remote origin` (the bare repo).
- The check is a small stub script outside the repos, e.g. `$tmp/check.sh`, passed after `--`. It
  appends `$(git rev-parse HEAD) $(pwd -P)` to `$tmp/ran`, then exits 1 if a file `RED` exists in its
  working directory, exits 1 once if `$tmp/flaky` exists (and deletes it), else exits 0.
- Helpers: `remote_ref <ref>` (`git -C remote.git rev-parse -q --verify <ref>` or empty), `push_commit
  <msg> [file to add or remove]` in `work`.
- Each case checks the exit code, the exact last line (`tail -n 1`) and the refs in `remote.git`.

Cases:

1. **Usage:** `--bogus`, `--remote` with no value, `--remote nosuch`, and `--` with nothing after it
   exit 2 with the usage line on stderr. `release-check.sh` with no commit, and with `nosuchref`, exit 2.
2. **First green:** no green ref; the check passes → `refs/green/main` equals `main`, last line `green
   <oid>`, `$tmp/ran` has one line whose oid is `main` and whose directory is not `work` (the check ran
   in a scratch worktree).
3. **Green is current:** run again → last line `green is current: <oid>`, `$tmp/ran` unchanged.
4. **A batch:** push two commits, run → green moves to the new `main` in one run, one check line.
5. **The caller's checkout is never checked:** with an uncommitted file in `work` and `work` checked
   out at an older commit, the check's recorded oid is the remote's `main`.
6. **The scratch worktree is gone afterwards,** on green (case 2) and on red (case 8):
   `git -C work worktree list` has only `work`.
7. **Flaky:** create `$tmp/flaky`, push a commit, run → exit 0, two check lines, the rerun line printed,
   green moves.
8. **Red, one commit:** push a commit adding `RED` → exit 5, two check lines, green unchanged,
   `refs/green/frozen` equals that commit, last line `red <head> culprit <head>`, and the line before it
   names the commit to revert or fix.
9. **Release check while frozen:** `release-check.sh <old green>` exits 1 with the frozen sentence.
10. **The fix unfreezes:** push a commit removing `RED`, run → exit 0, green equals `main`,
    `refs/green/frozen` gone, last line `releases unfrozen (freeze at <red commit> cleared)`.
11. **Red across several:** push two commits, the first adds `RED` → exit 6, last line
    `red <head> across 2 commits from <first> to <head>`, frozen equals head. Then fix (case 10's way)
    to clear it.
12. **Release check:** `release-check.sh main` → 0 (green); a commit pushed after green → 1 with the
    not-yet sentence; `release-check.sh` against a fresh bare remote with no green → 1 with the
    nothing-yet sentence; a tag pointing at a green commit → 0.
13. **Green not on main:** set `refs/green/main` in `remote.git` to a commit not on `main` (an orphan
    commit made with `git commit-tree`) → exit 1, refs unchanged, no check line.
14. **The lease holds:** a check stub that, mid-check, moves `refs/green/main` in `remote.git` to the
    commit under check (simulating another verify) → exit 0 with `covered: green is <oid>`; a variant that
    moves it to an unrelated older commit on main → exit 1 with the moved sentence, and green is what the
    stub set (the script never overwrote it).
15. **One at a time:** create the lock directory with the pid of a live process (`sleep 30 &`, killed in
    the trap) → `already running`, exit 0, no check line; with a dead pid → the run proceeds.
16. **Nothing but the two refs is pushed:** after all cases, `git -C remote.git for-each-ref
    --format='%(refname)'` lists only `refs/heads/main` and the `refs/green/*` refs the cases expect, and
    no tag the test didn't push.
17. **A strange check argument stays one argument:** pass `-- "$tmp/check.sh" 'a b;$(touch PWNED)'`
    (the stub ignores extra args) and assert no `PWNED` file appears anywhere under `$tmp`.
18. **Bash 3.2:** when `/bin/bash` reports major version 3, run case 4 again under `/bin/bash`.

Run time: seconds. No case may run the real `ci-local.sh`.

### `scripts/ci-local.sh` and `tests/ci_local_test.sh`

- Add `verify_main` to the end of `shell_suites`, so the full tier runs `bash tests/verify_main_test.sh`
  right after `bash tests/ci_local_test.sh`.
- Add `scripts/verify-main.sh scripts/release-check.sh tests/verify_main_test.sh` to the end of
  `shellcheck_line`.
- In the touched tier's `case` table, before the catch-all full rule, one rule with a one-line comment:
  `scripts/verify-main.sh`, `scripts/release-check.sh`, `tests/verify_main_test.sh` →
  `bash tests/verify_main_test.sh` only.
- Update `tests/ci_local_test.sh`'s golden lists (the shellcheck line and the full tier's step list) to
  match, and add those three paths to its per-path loop expecting `bash tests/verify_main_test.sh` and
  no `cargo test` line.

### Docs

- `.ai/PROJECT.md`'s table: one row each for `scripts/verify-main.sh` ("checks the remote's `main`
  with the full gate, moves `refs/green/main` on a pass, reruns a red once, and freezes releases with
  `refs/green/frozen` on a second red") and `scripts/release-check.sh` ("refuses a release of a commit
  that is not green, or any release while frozen"), both tested by `tests/verify_main_test.sh`.
- `CONTRIBUTING.md`: a short *Releasing* section, in plain words. Before tagging a release, run
  `bash scripts/release-check.sh <commit>`; it must say the commit passed. `main` is checked after
  lands by `bash scripts/verify-main.sh`, which runs the full gate on the newest `main` and moves
  `refs/green/main` when it passes. If `main` goes red twice, releases stay frozen until a fix passes.
  Say the version scheme already in use: semver `vX.Y.Z`, on the commit the build comes from, with the
  workspace version, `tauri.conf.json` and `builder/ui/package.json` bumped together.

## Not in scope

- No scheduler, job file or machine name anywhere in this repo. No change to how lands work, to
  `direct-to-main` or to any test other than `tests/ci_local_test.sh` and the new one.
- No new dependencies.

## How to check before you hand back

- `bash tests/verify_main_test.sh` red without the scripts (show the failure), then green.
- `bash tests/ci_local_test.sh` green.
- `shellcheck scripts/ci-local.sh scripts/verify-main.sh scripts/release-check.sh tests/ci_local_test.sh
  tests/verify_main_test.sh` clean.
- Don't run the full gate (it takes hours here); V1 runs it.
- Commit on the current branch (`lane-v1/PE3.3v`) with messages written to a file (`git commit -F`), in
  two commits: the test first (red), then the scripts, the gate wiring and the docs. Subjects:
  `tests: …` / `gate: …`, ending `(PE3.3v, lane: V1)`. No AI attribution or co-author trailers.
