# PE3.3v fix-up 1: the two new scripts also pick the crew installer test

Card `GhAdPMig`. Follows `plans/briefs/PE3.3v-verify-main.md`. Codex implements; V1 reviews and lands.

## What's wrong

`bash tests/ci_local_test.sh` is red on this branch:

```
ci_local_test: read shell crew_install_test <- scripts/read_fence_probe.py: picked
ci_local_test: missing step: bash tests/crew_install_test.sh
```

`tests/crew_install_test.sh` copies the whole `scripts/` folder (`cp -R "$here/scripts" "$src/scripts"`,
line 19), so its drift guard probes every file in `scripts/`, the two new ones included. The touched
tier's new rule maps `scripts/verify-main.sh` and `scripts/release-check.sh` to `verify_main` only, so a
change to either leaves out the crew installer test. Every other `scripts/` rule already adds
`crew_install` for this reason (`scripts/fetch-radio.sh`, `scripts/build-mac-app.sh`, `scripts/*.py`).

## The fix

1. `scripts/ci-local.sh`, the rule added in this branch: split it in two, matching the neighbouring rules'
   comments:
   - `scripts/verify-main.sh|scripts/release-check.sh` → `shell_picked+='verify_main crew_install '`, with
     the comment `# The main check and release guard test reads them; the crew installer test copies scripts/.`
   - `tests/verify_main_test.sh` → `shell_picked+='verify_main '` (with the other "each shell test
     exercises itself" lines).
2. `tests/ci_local_test.sh`, the per-path case this branch added: for the two scripts also
   `expect_step 'bash tests/crew_install_test.sh'`; for `tests/verify_main_test.sh` keep only
   `bash tests/verify_main_test.sh`. Both still `no_tests`.

Nothing else changes.

## How to check

- Show `bash tests/ci_local_test.sh` red first on the current branch (it is), then green after the fix.
- `bash tests/verify_main_test.sh` still green; `shellcheck scripts/ci-local.sh tests/ci_local_test.sh` clean.
- You can't commit in this sandbox: leave the change uncommitted and write the commit message to
  `/tmp/PE3.3v-fixup1-commit.txt`: `gate: the main check's scripts also pick the crew installer test
  (PE3.3v fix-up 1, lane: V1)`. No AI attribution or co-author trailers.
