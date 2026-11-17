# PE2.4v: voice's gate gets a touched tier (the changed crate and what depends on it)

Card `XKc5mkZ6`. Spec: lane V's spec file § PE2.4v, and the process-evaluation plan § PE2 (the plan wins).
Codex implements this brief; V1 (Claude) reviews, gates and lands it.

Read first, and follow: `AGENTS.md`, `.ai/PROJECT.md` (the path-to-test table), `CONTRIBUTING.md`,
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`.

## Why

`scripts/ci-local.sh` runs everything on every land. On today's loaded Mac that is about 3 hours
(VF22's land, 2026-11-17: 3 h 04 min), and a lane run has 2. A change to one crate should run that
crate's checks, the checks of every crate that depends on it, and the cheap scans that cover the whole
repo. Anything the tier can't map must fall back to the full gate, so a gap fails safe.

## What to build

Only two files change, plus one line in `.ai/PROJECT.md`'s table:

1. `scripts/ci-local.sh` gains options. Today's behaviour with no options must not change.
2. A new self-test, `tests/ci_local_test.sh`, red first.

### The command line

```
bash scripts/ci-local.sh                       # the full gate, as today (same steps, same order)
bash scripts/ci-local.sh --tier full           # the same
bash scripts/ci-local.sh --tier touched        # the touched tier (below)
bash scripts/ci-local.sh [--tier …] --list     # print the steps it would run, one per line, and run none
bash scripts/ci-local.sh --tier touched --base <rev>   # compare with <rev> instead of the merge base
```

- Unknown option, a missing value, or `--base` without `--tier touched`: print
  `Usage: scripts/ci-local.sh [--tier full|touched] [--base <rev>] [--list]` to stderr, exit 2.
- `--list` writes **only step lines to stdout**, one per step, each exactly the line the run echoes
  before that step (today's echo text, e.g. `cargo fmt --check`). Every note (skips, the fallback
  reason, which crates were picked) goes to **stderr**. `--list` runs no step: no test, no cargo
  build/test/clippy/tree, no shellcheck, no pnpm. It may run `git` and `cargo metadata` to work out the
  steps.
- A run echoes each step's line before running it, as today, and ends `ci-local: all passed`. Add one
  line at the start of a run (not in `--list` stdout): `ci-local: tier full, N steps` or
  `ci-local: tier touched, N steps (crates: a b c)` / `(no crates)`.

### Restructure into a step list

Build the steps first, then either print or run them. Suggested shape (keep it plain and readable,
matching the file's current style):

- A function appends a step: its label (the echo line) and how to run it.
- Simple steps run their literal command. Steps with logic become small functions, each with the
  label it prints today: the `cargo tree` feature checks (six calls; keep each `cargo tree` call as its own
  step with today's echo line and today's error text), the generated-types check (`git diff
  --exit-code -- builder/ui/src/types/generated` plus the untracked-file check, one step), the
  keystore cross-target `cargo check` per target (with today's skip line, printed to stderr at list
  time when the target is missing), and the four pnpm steps (with today's pnpm-missing and
  pnpm-too-old skip lines).
- If you use `eval` for literal steps, the strings may contain **only** literals written in the script
  and crate names that passed validation (`^[a-z0-9][a-z0-9_-]*$`; any other name from `cargo
  metadata`: print `ci-local: unexpected package name from cargo metadata` and exit 1). A path from
  git must never reach `eval`, a command string or an unquoted expansion.
- The Darwin/non-Darwin split for `VOICE_REQUIRE_SANDBOX=1` stays as today, in both tiers.
- `cargo` missing: the Rust steps are left out with today's skip line (stderr in `--list`). Same for
  pnpm.
- Must run on macOS's stock `/bin/bash` 3.2: no associative arrays, no `mapfile`/`readarray`, no
  `${x,,}`, no `declare -n`. `set -euo pipefail` stays. Empty arrays under `set -u` in 3.2 need the
  `${arr[@]+"${arr[@]}"}` form.

### The full tier

Exactly today's steps in today's order, plus two changes:

- add `bash tests/ci_local_test.sh` right after `bash tests/fetch_radio_test.sh`;
- add `tests/ci_local_test.sh` to the end of the `shellcheck` list.

### The touched tier

**Which paths changed.** `base` is `--base <rev>` when given, else `git merge-base origin/main HEAD`.
If neither works (no `origin/main`, no merge base, bad `--base`), fall back to the full tier with the
note `ci-local: can't tell what changed (<short reason>), running the full gate`. The changed paths
are the union of:

- `git diff --name-only -z --no-renames <base> HEAD --` (every commit on the branch), and
- `git diff --name-only -z --no-renames HEAD --` (uncommitted changes to tracked files), and
- `git ls-files -z --others --exclude-standard` (new files not yet added).

Read them NUL-separated (`while IFS= read -r -d '' path`), so a file name with a space, a newline or
a shell character is just a string that fails to match the rules below (and so falls back to full).
No changed paths at all: only the always steps run.

**The rules**, first match wins, on the repo-relative path. Write them as one `case` table in the
script, in this order, with a one-line comment per rule saying why (who reads that path):

| # | Path pattern | Adds |
|---|---|---|
| 1 | `.ai/*`, `plans/*`, `docs/spikes/*`, `README.md`, `CONTRIBUTING.md`, `AGENTS.md`, `SECURITY.md` | nothing (records and prose no test reads) |
| 2 | `scripts/ci-local.sh`, `tests/ci_local_test.sh` | **full** (a change to the gate runs the whole gate) |
| 3 | `builder/ui/src/types/generated/*` | crate `voice-api` (its ts-rs exports write them), the screens |
| 4 | `builder/ui/src/screens/build/keepGoing.ts` | crate `voice-api` (`builder/api/tests/building.rs` reads it), the screens |
| 5 | `builder/ui/*` | the screens |
| 6 | `builder/cli/tests/anonymize.rs` | crates `voice-cli` and `voice-ingest` (`builder/ingest/tests/anonymize_file.rs` includes it) |
| 7 | `builder/core/src/deny.rs` | crate `voice-core`, Python suite `read_fence_probe` (`tests/read_fence_probe_test.py` reads it) |
| 8 | `builder/grow/tests/golden/*` | crate `voice-grow`, Python suite `growth_loop_spec` (it reads the golden files) |
| 9 | `builder/install/*` | `bash tests/mac_install_test.sh` |
| 10 | `builder/*` inside a workspace crate | that crate (the nearest `Cargo.toml` above the file that has a `[package]` section, at or below `builder/<dir>/`; read its `name`) |
| 11 | `docs/spec/*` | crate `voice-grow` (its tests read the examples), Python suite `growth_loop_spec` |
| 12 | `scripts/crew_*.py`, `tests/crew_*.py` | crate `voice-core` (Rust parity tests in several crates run `scripts/crew_decisions.py` and read the `tests/crew_*_fixtures.py` files), every Python suite, `bash tests/crew_install_test.sh` |
| 13 | `scripts/*.py`, `tests/*.py` | every Python suite (the tests import each other and the scripts) |
| 14 | `bootstrap/*` | `bash tests/crew_install_test.sh` |
| 15 | `scripts/fetch-radio.sh` | `bash tests/fetch_radio_test.sh` |
| 16 | `tests/crew_install_test.sh`, `tests/mac_install_test.sh`, `tests/fetch_radio_test.sh` | that test |
| 17 | anything else (`Cargo.toml`, `Cargo.lock`, `.cargo/*`, `.gitignore`, `crew/*`, `assets/*`, `LICENSE`, `scripts/build-mac-app.sh`, a `builder/*` path in no crate, …) | **full** |

A **full** match anywhere means the whole tier is the full tier: print to stderr
`ci-local: tier touched falls back to the full gate: <path> maps to no part` (for rule 2 say
`… : <path> is the gate itself`), naming the first such path only. In `--list` stdout, print the full
tier's steps.

**Dependents.** The crate set is the crates the rules picked, plus every workspace crate that depends
on one of them, transitively, through normal, dev or build dependencies (ignore a crate's dependency on
itself). Read the graph with `cargo metadata --locked --offline --no-deps --format-version 1` and a
short `python3 -I -c` (stdlib `json` only) that takes the picked names as arguments and prints the
closure, one name per line, sorted. Python 3.9 compatible. If `cargo metadata` fails, fall back to full
with a note. Today's graph, for your own checking: `voice-core` is under everything but
`voice-keystore`; `voice-keystore` → `voice-save` → `voice-agent` → most; `voice-panel-runner` →
`voice-api` → `voice-cli`, `voice-app`; `voice-app` has no dependents.

**The touched tier's steps**, in this order (fast first). Labels are the exact echo lines; `<C>` is the
crate set as `-p a -p b …` in sorted order; `<F>` is `<C>` limited to `voice-agent`, `voice-api`,
`voice-cli` (the crates with the `claude-subscription` feature that today's feature test covers):

1. Always: `shellcheck <today's list plus tests/ci_local_test.sh>` (same label as the full tier).
2. Picked Python suites, in the full tier's order: `python3 tests/<t>_test.py`.
3. Picked shell tests, in the full tier's order.
4. Cargo present, always: `cargo fmt --check`, then the six `cargo tree` calls of the feature checks, exactly as the
   full tier (they resolve, they don't build).
5. Cargo present and `<C>` not empty:
   - `<F>` not empty: `VOICE_REQUIRE_PARITY=1 [VOICE_REQUIRE_SANDBOX=1 ]cargo test --locked <F> --features claude-subscription`
   - `VOICE_REQUIRE_PARITY=1 [VOICE_REQUIRE_SANDBOX=1 ]cargo test --locked <C>`
   - `cargo clippy --locked <C> --all-targets -- -D warnings`
   - `<C>` limited to `voice-agent voice-api voice-cli voice-app`, if not empty:
     `cargo clippy --locked <that> --all-targets --features claude-subscription -- -D warnings`
   - `<C>` limited to `voice-cli voice-app`, if not empty: `cargo clippy --locked <that> -- -D warnings`
   - `voice-keystore` in `<C>`: the keystore cross-target checks, as the full tier.
6. Cargo present, always: the generated-types check (same label and errors as the full tier).
7. The screens picked: the four pnpm steps with today's version gate.

The always steps (1, 4, 6) are the repo-wide scans. Name them in a comment above the touched-tier code.

## The self-test: `tests/ci_local_test.sh`

Bash, `set -euo pipefail`, finds the repo from its own location (no fixed paths), cleans up after itself
with a `trap` (remove the temp worktree with `git worktree remove --force` and its directory).
Write it first and show it red against today's `ci-local.sh` (no `--tier`): the record needs that.

Setup: `git worktree add --detach "$tmp/wt" HEAD` of the repo under test; copy the working tree's
`scripts/ci-local.sh` into it and commit it there (`git -c user.name=t -c user.email=t@example.invalid
commit -qm base`), so the copy under test is the baseline. For each case: make a change in the temp
worktree, commit it, run `bash scripts/ci-local.sh --tier touched --base HEAD~1 --list` from there with
stdout and stderr captured separately, check, then `git reset -q --hard HEAD~1`. Never touch the
caller's checkout.

**Proving `--list` runs nothing:** put a stub directory first on `PATH` with `shellcheck`, `pnpm`
and `python3`-free stubs for `shellcheck` and `pnpm` that append their name to a marker file, and a
`cargo` stub that appends its argv to the marker unless its first argument is `metadata`, in which case
it `exec`s the real cargo (found before the PATH change). After every `--list` call the marker must be
empty. (Skip the cargo stub, with a printed line, when no real cargo exists; the Rust cases then check
the cargo-missing skip line instead.)

Cases (each checks stdout lines exactly where a step is named; use `grep -Fx`):

1. **Usage:** `--tier bogus` and `--base x` without `--tier touched` exit 2 with the usage line on stderr.
2. **Full list is today's gate:** `--list` (no tier) and `--tier full --list` print the same lines, and
   they contain, in order, each of today's step labels (write today's list into the test as the golden
   list, with the two additions), on Darwin with cargo and pnpm present; on other hosts check only the
   non-Rust prefix.
3. **A crate change picks its dependents:** a change to `builder/panel-runner/src/lib.rs` (append a
   comment) lists `cargo test --locked -p voice-api -p voice-app -p voice-cli -p voice-panel-runner`
   (with the Darwin env prefix where applicable) and no `-p voice-core`.
4. **A leaf crate:** `builder/app/build.rs` lists `-p voice-app` only in the cargo test line, and the
   `voice-cli voice-app` clippy line becomes `cargo clippy --locked -p voice-app -- -D warnings`.
5. **voice-keystore:** lists the keystore cross-target step(s) (or their skip line on stderr).
6. **The repo-wide scans are always there:** for a `.ai/` only change, stdout is exactly the shellcheck
   line, `cargo fmt --check`, the six `cargo tree` lines and the generated-types line, and nothing else.
7. **The screens:** `builder/ui/src/App.vue` (or any existing file under `builder/ui/src`) lists the four
   pnpm lines (or the pnpm skip line on stderr) and no `cargo test` line.
8. **A gap fails safe:** a new file `newpart/x.txt` makes stdout equal to the full tier's `--list`, and
   stderr names `newpart/x.txt`. Same for `Cargo.toml`, `crew/_template/member.json` and the gate itself
   (`scripts/ci-local.sh`: append a comment and commit; stderr says it is the gate itself).
9. **A strange file name** (`builder/core/src/a b;$(touch PWNED).rs`, created and committed) falls back
   to full or maps to voice-core, and no `PWNED` file appears anywhere in the temp worktree.
10. **Python:** `tests/slack_select_test.py` lists every Python suite and no `cargo test` line;
    `scripts/crew_decisions.py` lists every Python suite, `bash tests/crew_install_test.sh` and a cargo
    test line containing `-p voice-core`.
11. **Uncommitted changes count:** an uncommitted edit to `builder/app/build.rs` (no commit, `--base
    HEAD`) lists `-p voice-app`; an untracked new file `builder/app/src/new.rs` does too.
12. **Bash 3.2:** when `/bin/bash` reports version 3, run case 3 again under `/bin/bash`.
13. **The cross-crate reads stay mapped (drift guard):** scan `builder/*/src` and `builder/*/tests`
    `*.rs` files for `include_str!("../…")` / `include_bytes!("../…")` and
    `env!("CARGO_MANIFEST_DIR")` followed on the same line by `.join("../…")` with a string literal;
    resolve each target to a repo path relative to the file (or to the crate dir for
    `CARGO_MANIFEST_DIR`); for each target that is **outside the reading file's own crate**, the
    touched `--list` for a change to that target path must either be the full list or contain the
    reading crate in its cargo test line. Do the scanning in a `python3 -I` heredoc. Targets that are
    directories (`crew/_template`) or format strings (`{kind}`) use their fixed prefix as the path to
    test. Skip, with a counted line, targets that don't exist. Print how many reads were checked.

Run time matters: each case only lists, so the test should take seconds plus one worktree checkout.

## Docs

- `.ai/PROJECT.md`'s table: add a row for `scripts/ci-local.sh`: "The gate: `--tier full` (default)
  runs everything; `--tier touched` runs the changed crates and their dependents plus the repo-wide
  scans, and falls back to full for anything it can't map; `--list` prints the steps" | test
  `tests/ci_local_test.sh`. Change the sentence under the table that says "All of it runs with `bash
  scripts/ci-local.sh`" only to add: "`--tier touched` runs only what a change touches."
- A short comment block at the top of `ci-local.sh` with the usage lines above.

## Not in scope

- Don't change which steps the full tier runs (beyond the two additions), the lane's land command,
  `direct-to-main.sh`, or any test outside `tests/ci_local_test.sh`.
- Don't add dependencies. Bash, git, cargo, python3 stdlib only.

## How to check before you hand back

- `bash tests/ci_local_test.sh` red against the old script (show the failure), then green.
- `shellcheck scripts/ci-local.sh tests/ci_local_test.sh` clean.
- `bash scripts/ci-local.sh --list` and `--tier touched --list` (on your branch, against `origin/main`)
  print what you expect; paste both outputs in your summary.
- Don't run the full gate (it takes hours on this machine); V1 runs it.
- Commit on the current branch (`lane-v1/PE2.4v`) with messages written to a file (`git commit -F`),
  in two commits: the test first (red), then the script and docs. Subject style: `gate: …` /
  `tests: …`, ending `(PE2.4v, lane: V1)`. No AI attribution or co-author trailers.
