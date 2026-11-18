# PE3.3v fix-up 4: the holder check never reads a pipe's SIGPIPE as "not running"

Card `GhAdPMig`. Follows fix-up 3. Codex implements; V1 reviews and lands.

`scripts/verify-main.sh` runs under `set -o pipefail`, and fix-up 3 checks the lock holder with
`ps -p "$pid" -o command= | grep -q 'verify-main\.sh'`. `grep -q` exits at its first match; if `ps` is still
writing then, it dies of SIGPIPE, `pipefail` makes the whole test false, and a live run is called stale and
its lock removed.

- Replace the pipe with a captured string and a pattern match, no pipe:
  ```bash
  holder=$(ps -p "$pid" -o command= 2>/dev/null || true)
  ```
  and require `[[ "$holder" == *verify-main.sh* ]]` beside the existing `kill -0` check. Declare nothing
  else; keep the rest of the lock block as it is.
- No new test: the race can't be forced. The existing live-holder, reused-pid and race cases must stay green.

## How to check

- `bash tests/verify_main_test.sh` green (it needs `ps`; if your sandbox denies it, say so and stop there).
- `shellcheck scripts/verify-main.sh` clean.
- Leave the change uncommitted; write the message to `/tmp/PE3.3v-fixup4-commit.txt`:
  `gate: the lock holder's command is read whole, so a pipe can't call a live run stale (PE3.3v fix-up 4,
  lane: V1)`. No AI attribution or co-author trailers.
