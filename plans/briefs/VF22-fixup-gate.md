# VF22 gate fix-up: the build test joins the gate PE2.4v rewrote

Lane V, runner V1, 2026-11-17. Card `lMRUNtZ8`. VF22 (`tests/build_mac_app_test.sh`, the universal build on a
case-insensitive disk) was built before PE2.4v landed. PE2.4v rewrote `scripts/ci-local.sh` around suite lists, a
touched tier and a drift guard (`tests/ci_local_test.sh`), so VF22's two old lines (run the test, shellcheck it) were
dropped in the rebase. This puts the test into the new shape.

## What to change

Only `scripts/ci-local.sh` and `tests/ci_local_test.sh`.

### `scripts/ci-local.sh`

1. `shell_suites`: add `build_mac_app` after `fetch_radio` (before `ci_local`).
2. `shellcheck_line`: add `tests/build_mac_app_test.sh` before `tests/ci_local_test.sh`.
3. In `map_path`, so every file the build test reads picks it in the touched tier
   (`tests/build_mac_app_test.sh` copies `scripts/build-mac-app.sh`, `scripts/fetch-radio.sh`,
   `builder/install/install.sh` and `builder/install/How to install.txt`):
   - `builder/install/*)` picks `mac_install fetch_radio build_mac_app`; update its comment to name the build test.
   - `scripts/fetch-radio.sh)` picks `fetch_radio crew_install build_mac_app`; update its comment.
   - Add `scripts/build-mac-app.sh)` picking `build_mac_app`, with a comment. If the drift guard reports another shell
     test reading `scripts/build-mac-app.sh`, add that suite to this line too.
   - Add `tests/build_mac_app_test.sh) shell_picked+='build_mac_app ' ;;` beside the other "each shell test exercises
     itself" lines.
   Keep every other line as it is.

### `tests/ci_local_test.sh`

1. The block that lists the full gate's expected steps (the `{ … echo 'bash tests/fetch_radio_test.sh' … }` group):
   add `echo 'bash tests/build_mac_app_test.sh'` after the `fetch_radio` line, matching the order `shell_suites` gives.
2. In the loop whose `case` has `builder/install/*)`: also `expect_step 'bash tests/build_mac_app_test.sh'`.
3. In the loop whose `case` has `scripts/fetch-radio.sh)`: also `expect_step 'bash tests/build_mac_app_test.sh'`.
4. Add `scripts/build-mac-app.sh` and `tests/build_mac_app_test.sh` to that second loop's path list (the one that also
   fails on an unexpected full gate), each with a `case` arm expecting `bash tests/build_mac_app_test.sh`.

## Tests first

Make the `tests/ci_local_test.sh` changes first and run `bash tests/ci_local_test.sh`; it must fail (RED lines naming
the missing build test step). Then change `scripts/ci-local.sh` and run it again until it passes.

## Run (report each exit code)

```
bash tests/ci_local_test.sh
bash tests/build_mac_app_test.sh
shellcheck scripts/ci-local.sh bootstrap/install.sh tests/crew_install_test.sh scripts/build-mac-app.sh builder/install/install.sh tests/mac_install_test.sh scripts/fetch-radio.sh tests/fetch_radio_test.sh tests/build_mac_app_test.sh tests/ci_local_test.sh
bash scripts/ci-local.sh --tier touched --list
```

Don't run the full `scripts/ci-local.sh`. Don't commit. No other file, no real person's name, handle or path.

## Skills

`~/.codex/skills/tdd-skill/SKILL.md` (Claude: `zizi-skills:tdd-skill`).

## Fix-up 2 (review of the first pass)

The first pass also changed the drift guard's read finder in `tests/ci_local_test.sh` (an `anchors` choice made by
sniffing the test's text for `repo="$scratch/repo"`, and a regex that needs a closing quote). Replace that with:

1. In `tests/build_mac_app_test.sh`, rename the scratch copy's variable from `repo` to `work` everywhere (the
   assignment `repo="$scratch/repo"` becomes `work="$scratch/repo"`, and every `$repo` / `"$repo/…"` becomes
   `$work` / `"$work/…"`). The guard's convention is that `$here` and `$repo` name the real repository; the build
   test's scratch copy must not use either name. Change nothing else in that file.
2. In `tests/ci_local_test.sh`, restore the shell read finder to one line with no `anchors` sniff:
   `for target in re.findall(r'"\$(?:here|repo)/([^"$]+)', source.read_text()):` (the old one stopped at
   whitespace, which cut `"$here/builder/install/How to install.txt"` short; this stops only at a quote or `$`).
   Remove the `text =` and `anchors =` lines and the comment above them.

Checked by V1 before this brief: with both changes, every other `tests/*_test.sh` yields exactly the reads the old
finder did, and the build test yields `scripts/build-mac-app.sh`, `scripts/fetch-radio.sh`,
`builder/install/install.sh` and `builder/install/How to install.txt`.

Run the same four commands as above and report each exit code. Don't commit.

## Fix-up 3 (the Verifier's should-fix)

`tests/build_mac_app_test.sh` makes its scratch repository with `git init -q` and commits with only `user.name` and
`user.email` set, so a contributor's global `commit.gpgsign=true`, `core.hooksPath` or `init.templateDir` runs or
fails there. Change only those lines (about 42 to 44), in the style `tests/ci_local_test.sh` line 15 already uses:

- `git init -q` becomes `git -c init.templateDir= init -q`;
- the commit becomes `git -c core.hooksPath=/dev/null -c commit.gpgsign=false -c user.name=test -c
  user.email=test@example.invalid commit -q -m 'build test copy'` (one line).

Run `bash tests/build_mac_app_test.sh` and `shellcheck tests/build_mac_app_test.sh`; report each exit code. Don't
commit.
