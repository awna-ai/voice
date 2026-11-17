# PB8.9d1 fix-up 4: `voice radio invite` never drops the recovery line

The repeat Verifier (Opus, PASS WITH FIXES) on `099a446`. In `builder/cli/src/radio.rs`, the `Invite` arm
runs `local_time(invite.expires_at)?` before it prints the recovery sentence, so a failed time read after a
fresh `fleet_init` exits with an error and the recovery key's path is never shown again (Radio names it once).

1. Split the printing into `fn invite_lines(invite: &dto::Invite) -> Vec<String>` that cannot fail:
   - `local_time` answers `Option<String>` instead of `Result` (`None` when `localtime_r` fails).
   - The lines are: the address line, `Join code: <code>`, then `It works once, until <HH:MM>.` or, when the
     time can't be read, `It works once, for a short while.`, then the recovery sentence when there is one.
   - The `Invite` arm prints each line.
2. Tests (in this file's existing test module, or a new `#[cfg(test)] mod tests` beside it if there is none):
   `invite_lines` with `expires_at: 1e300` (the time can't be read) and a recovery file still ends with
   `text::recovery(<that path>)` and says `It works once, for a short while.`; with a normal time it says
   `until` and the recovery line comes last.

Don't run cargo or pnpm; the reviewer runs every check. No new dependency. Commit nothing; list the files you
changed.
