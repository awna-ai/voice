# PE2.4v fix-up 2: the drift guard fails on what it can't resolve, and probes every file of a folder

Card `XKc5mkZ6`. Follows `plans/briefs/PE2.4v-touched-tier.md` and `plans/briefs/PE2.4v-fixup-1.md` (read
both first). The repeat Verifier passed `1d36b35` with two should-fixes, both in the self-test's drift guard
(`tests/ci_local_test.sh`, the `python3 -I - "$repo" <<'PY'` scan and the loop after it). Change
`tests/ci_local_test.sh`, and `scripts/ci-local.sh`'s `map_path` only where step 3 says. Nothing else.

## 1. A read the guard can't resolve fails it, unless it is on a fixed list

Today `add()` counts `skipped` for a target outside the repo, a missing path or a folder with no tracked
file, and prints only the count. Change it:

- A target that resolves to the repo root itself (the `../..` base of a `repo()` helper, read by
  voice-cli, voice-core, voice-ingest and voice-steps) is a base, not a read: drop it, uncounted.
- Every other unresolved read is recorded as `(kind, reader, target)`, with `target` exactly as the regex
  captured it.
- A fixed list in the scan, `ALLOWED_UNRESOLVED`, holds the ones known to be harmless, each with a
  one-line comment saying why:
  - `('rust', 'voice-core', 'crew/ari')`, `('rust', 'voice-core', 'crew/mo-lee')`,
    `('rust', 'voice-core', 'crew/panels')`: scratch-folder paths in `builder/core/src/install_tests.rs`
    (`scratch.path().join(...)`), not repo files.
  - `('python', 'claude_sandbox_probe_test', 'src/sandbox.rs')`: read through
    `probe.PROFILE.parent.parent`, so it is `builder/agent/src/sandbox.rs`, which fix-up 1's rule and its
    own self-test case already cover.
- After the scan, print each unresolved read on stderr as `ci_local_test: unresolved <kind> <reader> <target>`.
  Exit the scan with status 1 if any is not in `ALLOWED_UNRESOLVED`, naming it, and also if any entry
  of `ALLOWED_UNRESOLVED` was not seen (a stale entry), naming it. The script runs `set -euo pipefail`,
  so a non-zero scan already stops it; add `|| fail 'drift guard: unresolved reads (see above)'` to the
  scan's line so the failure says why.
- Replace the `skipped (missing): N` line with `ci_local_test: unresolved reads: N, all allowed`.

## 2. A folder read probes every tracked file in it, not the first

Today a folder target is replaced by the first tracked file under it. Instead add one read per tracked
regular file under the folder:

- for `kind == 'python'`, only tracked `*.py` files under it (a Python suite reaches a folder by putting it
  on `sys.path` or running a named script, so only `.py` files can reach it; say so in a comment);
- for `rust` and `shell`, every tracked file under it (Rust `include_*!`/`repo().join` and the shell
  tests' `cp -R` read any file).

A folder with no file left after that filter is unresolved (step 1's rules apply).

## 3. The table gaps step 2 exposes

`tests/crew_install_test.sh` copies the whole of `scripts/` into its scratch install (`cp -R
"$here/scripts"`), so a change to any file in `scripts/` must list `bash tests/crew_install_test.sh` or
fall to the full gate. Today `scripts/*.py` (other than `scripts/crew_*.py`) and `scripts/fetch-radio.sh`
don't. In `map_path`, keeping one comment line per rule:

- split `scripts/*.py|tests/*.py)` into
  `scripts/*.py) python_picked=" $python_suites "; shell_picked+='crew_install ' ;;` and
  `tests/*.py) python_picked=" $python_suites " ;;`, with comments saying the crew installer test copies
  `scripts/`;
- change `scripts/fetch-radio.sh)` to `shell_picked+='fetch_radio crew_install ' ;;`.

Then run the self-test. If any other probe fails, fix it the same way (the narrowest rule that picks the
reader, or send the path to full), and list it in your summary. Do not add entries to
`ALLOWED_UNRESOLVED` beyond the four above without saying why in your summary.

## Self-test cases

Add listing cases next to fix-up 1's: `scripts/slack_select.py` lists `bash tests/crew_install_test.sh`
and every Python suite; `scripts/fetch-radio.sh` lists `bash tests/fetch_radio_test.sh` and
`bash tests/crew_install_test.sh`; `tests/slack_mine_test.py` does not list `bash tests/crew_install_test.sh`.

## Proof (run each, paste the tail of each in your summary)

- `bash tests/ci_local_test.sh` passes; its last lines show the new unresolved line and the read counts.
- `shellcheck scripts/ci-local.sh tests/ci_local_test.sh` is clean.
- Do not commit: the worktree's git index is not writable from your sandbox; V1 commits.

## Rules

- Bash 3.2 compatible, as the rest of the script (the self-test runs a Bash 3.2 pass).
- No new dependencies, no fixed paths, no real people's names anywhere (invented fixtures only).
- Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.
