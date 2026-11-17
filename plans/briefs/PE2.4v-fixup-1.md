# PE2.4v fix-up 1: the touched tier's table misses three readers

Card `XKc5mkZ6`. Follows `plans/briefs/PE2.4v-touched-tier.md` (read it first). The security review
found readers the rule table doesn't pick, each verified by listing the touched steps for a one-byte
change. Change `scripts/ci-local.sh`'s `map_path` table, `tests/ci_local_test.sh`, and the brief's
own table text is not edited (this file records the change).

## The table changes (in `map_path`, keeping one comment line per rule)

1. **The sandbox's Rust and Python halves.** `tests/claude_sandbox_probe_test.py`
   (`test_helper_matches_rust_and_is_private`) reads `builder/agent/src/sandbox.rs`, and
   `scripts/claude_sandbox_probe.py` uses `builder/agent/sandbox/claude-code.sb` as its profile. Add,
   before the generic `builder/*` rule:
   `builder/agent/src/sandbox.rs|builder/agent/sandbox/*) pick voice-agent; python_picked+='claude_sandbox_probe read_fence_probe ' ;;`
   (`read_fence_probe` imports `claude_sandbox_probe`, so it runs too.)
2. **`bootstrap/` is also read by Rust.** `builder/core/tests/install_parity.rs` copies `scripts`,
   `bootstrap`, `crew/_template` and `.gitignore` and runs `bootstrap/install.sh`. Change the rule to
   `bootstrap/*) pick voice-core; shell_picked+='crew_install ' ;;`.
3. **The Radio pin.** `tests/fetch_radio_test.sh` checks `builder/install/radio-release.txt`. Change
   the rule to `builder/install/*) shell_picked+='mac_install fetch_radio ' ;;`.
4. **Find the rest yourself before you stop.** Grep every Python test (`tests/*.py`: `REPO /`,
   `Path(__file__)`, `parents[`), every shell test (`tests/*.sh`: `$here/`, `$repo/`) and every Rust
   test and `build.rs` (`repo().join(`, `#[path`, multi-line `CARGO_MANIFEST_DIR` joins, `format!`
   paths) for reads of repo files. For each read, check what `--tier touched --list` lists for a change
   to that file (use a throwaway worktree, as the self-test does). Fix any reader the table misses
   with the narrowest rule that picks it, or send the path to full. List every read you checked and
   its result in your summary.

## The self-test

- Add one case per change above: `builder/agent/src/sandbox.rs` lists
  `python3 tests/claude_sandbox_probe_test.py` and `python3 tests/read_fence_probe_test.py` and a
  cargo test line with `-p voice-agent`; `bootstrap/install.sh` lists `bash tests/crew_install_test.sh`
  and a cargo test line with `-p voice-core`; `builder/install/radio-release.txt` lists both
  `bash tests/mac_install_test.sh` and `bash tests/fetch_radio_test.sh`. Plus a case for each further
  reader you fix in step 4.
- The test's commits must not run a contributor's own hooks or ask to sign: make `commit()` use
  `git -c core.hooksPath=/dev/null -c commit.gpgsign=false -c user.name=t -c user.email=t@example.invalid commit …`.

## Proof

- Each new case red against the current script (show it), green after.
- `bash tests/ci_local_test.sh` green; `shellcheck scripts/ci-local.sh tests/ci_local_test.sh` clean.
- Don't run the full gate. Don't commit (your sandbox can't write the git index); V1 commits.

## Added after the Verifier's pass (FAIL at `25f5245`; her must-fix 1 to 3 are the three table changes above)

5. **The drift guard runs in the touched tier** (her must-fix 4). Add `bash tests/ci_local_test.sh` to
   the touched tier as an always step, right after the shellcheck line, so a new cross-crate read is
   caught on the land that adds it. Update the self-test's expected always list (the `.ai/` case).
6. **The drift guard sees more readers** (her should-fix 5). In the guard's `python3 -I` heredoc:
   - Rust, in every `builder/*/{src,tests}/**/*.rs`: besides today's two patterns, `#[path = "…"]`
     (resolved from the file's folder); `include!("…")`; `.join(format!("../…` (use the literal prefix
     before the first `{`); and, in a file that contains `repo()` or `join("../..")`, every string
     literal that is a repo-relative path starting `scripts/`, `bootstrap/`, `crew/`, `docs/`, `tests/`
     or `builder/`, or is exactly `.gitignore` (resolved from the repo root).
   - Python, in every `tests/*.py`: string literals after `REPO /`, `ROOT /`, `parents[1] /` or
     `parent.parent /` (single or double quotes), resolved from the repo root. The reader is that suite
     when the file is `tests/<x>_test.py`; for any other `.py` file, at least one Python suite must be
     listed.
   - Shell, in every `tests/*_test.sh`: `"$here/<path>"` literals (stop at `"`, `$` or a space),
     resolved from the repo root; the reader is that shell test.
   - A target that is a directory is probed with the first file `git ls-files` lists inside it, never
     an invented file name (an invented name always falls to full, so it proved nothing).
   - Reads inside the reader's own crate are skipped, as now. For each other read, the touched list for
     a change to the target must be the full list or contain the reader (`-p <crate>` in a `cargo test`
     line, `python3 tests/<x>_test.py`, or `bash tests/<x>_test.sh`).
   - Print the counts by kind (`rust`, `python`, `shell`) and how many fell to full.
   If this finds more gaps, fix them in the table (step 4 above) rather than weakening the guard.
7. **`--list` runs nothing, for every step** (her should-fix 6). In the temp worktree's base commit,
   replace every Python suite `tests/<x>_test.py` and every shell test `tests/<x>_test.sh` (including
   `ci_local_test.sh`'s copy there) with a stub that appends its name to the marker file and exits 0.
   The checks still read the real files from the caller's repo, so nothing else changes.
8. **An empty change set says so** (her should-fix 7). When the touched tier finds no changed paths,
   print to stderr `ci-local: no changed paths since <short base sha>, running only the repo-wide scans`.
   Add a self-test case (`--base HEAD` on a clean tree) that checks the note.

Time box: this run has about 45 minutes left. Do 1 to 3, 5, 7 and 8 first, then 6, then 4's grep.
