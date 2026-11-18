# PE3.3v fix-up 6: "covered" wins over "the freeze moved"

Card `GhAdPMig`. Follows fix-up 5. Codex implements; V1 reviews and lands.

In `scripts/verify-main.sh`, after a rejected green push, fix-up 5 checks `[ "$frozen" = "$checked_frozen" ]`
right after `read_refs`, before the "covered" check. If another run greened the same head and cleared the freeze
during this run's check, this run now exits 1 ("the freeze moved") where it used to say `covered: green is <green>`
and exit 0.

- Move that one check to just before the final `fail 'green moved during the check …'` line, after the covered
  branch. Covered then wins; only the path that would have said "green moved" says "the freeze moved".
- Test first: green at `G`, freeze at a commit `F` on `main` after `G`, then one more commit `H` pushed. The stub
  check (a new marker file in `$VERIFY_TEST_TMP`, say `cover`) sets the remote's `refs/green/main` to `H` and
  deletes `refs/green/frozen`. Verify: exit 0, last line `covered: green is <H>`. Before the move it exits 1.
  Clean up the marker after.
- Leave the change uncommitted; write the message to `/tmp/PE3.3v-fixup6-commit.txt`: `gate: a run another run
  already covered says so even when that run cleared the freeze (PE3.3v fix-up 6, lane: V1)`. No AI attribution.
- Check: `bash tests/verify_main_test.sh` (say which cases your sandbox can't run), shellcheck clean.
