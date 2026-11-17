# VF23 brief: the lasting-place rule refuses a group-writable folder, as Radio does

Lane V, runner V1, 2026-11-17. Card `4b5XoAh2`. Filed from VO16.6a's security review (low).

## Why

`builder/core/src/cli_command.rs` `lasting` decides whether Voice's own `voice` command sits in a place that
lasts. Since VO16.6a, Voice hands that command to Radio as its handler and runs the `radio` beside it at every
start. `lasting` today refuses a command or folder owned by another user, or writable by everyone
(`mode & 0o002`), but passes one writable by its group, whatever the group. Every local account on a Mac is in
`staff`, so a `staff`-writable `~/Applications` or bundle folder lets another account swap the binary.

Radio's own handler check (`crates/radio/src/install.rs` in the Radio repo, `validate_handler` and
`handler_folder_safe`) is stricter:

- the handler file itself: owned by the person or root, and `mode & 0o022 == 0` (not writable by group or
  others, whatever the group);
- each folder above it: owned by the person or root, writable by its group only when the group is wheel (gid 0)
  or admin (gid 80), and not writable by others (Radio allows a sticky world-writable folder; Voice keeps
  refusing every world-writable folder, which is stricter and stays).

Voice must match the first two rules. `/Applications` is `root:admin 0775` on a Mac and must still pass.

## What to change

Only these files:

1. `builder/core/src/cli_command.rs`
2. `.ai/SECURITY.md`

### 1. `cli_command.rs`

Split the ownership and mode test out of the loop in `lasting` into two small pure functions, `#[cfg(unix)]`,
private to the module, next to `on_a_passing_disk`:

```rust
/// The command itself: owned by the person or root, and writable by no one else (Radio's handler rule).
#[cfg(unix)]
fn command_safe(owner: u32, mode: u32, euid: u32) -> bool

/// A folder above the command: owned by the person or root, writable by its group only when the group is
/// wheel (0) or admin (80), as Radio's handler check allows, and never by others.
#[cfg(unix)]
fn folder_safe(owner: u32, group: u32, mode: u32, euid: u32) -> bool
```

- `command_safe`: `(owner == euid || owner == 0) && mode & 0o022 == 0`.
- `folder_safe`: `(owner == euid || owner == 0) && (mode & 0o020 == 0 || matches!(group, 0 | 80)) && mode & 0o002 == 0`.

In `lasting`, the loop over `canonical.ancestors()` uses `command_safe(metadata.uid(), metadata.mode(), euid)`
for the first item (the canonical path itself) and `folder_safe(metadata.uid(), metadata.gid(),
metadata.mode(), euid)` for every later one. Refuse with `Error::Invalid(CLI_PLACE.into())` as today. Keep the
`symlink_metadata` call, the error mapping and everything else in the function as it is. Use
`ancestors().enumerate()` or `ancestors().skip(1)` after a separate check of `canonical`; either is fine, but
read `symlink_metadata` once per path.

Update the doc comment text nowhere else; `lasting` has no doc comment today, so add none.

### 2. `.ai/SECURITY.md`

Two places, plain language, matching the file's style (no new section):

- The paragraph around line 74 that starts "A release build writes it only from a place that lasts": replace
  "the command and every folder above it must be owned by the person or root and not writable by others" with
  "the command and every folder above it must be owned by the person or root; the command may not be writable
  by its group or by others, and a folder may be writable by its group only when that group is wheel or admin
  (as `/Applications` is), never by others (Radio's own rule for a handler)". In its *Known limits*, delete
  "a folder writable by its group (as `/Applications` is, for admins) is allowed;" and nothing else.
- The Radio row (the table row starting "Voice installing Radio and talking to it"): change "or under a folder
  another user can write" to "or under a folder another user can write, a group-writable one counting unless
  its group is wheel or admin". Leave the rest of the row unchanged.

## Tests first (red, then green)

All in the existing `#[cfg(test)] mod tests` of `cli_command.rs`, in its style (plain `assert!`, `matches!`,
`Scratch`). Write them before the change and run them to see them fail on their assertions.

1. `#[cfg(unix)] fn the_command_is_writable_by_no_one_else()`: literal cases on `command_safe` with `euid = 501`:
   `(501, 0o755)` true, `(0, 0o755)` true, `(502, 0o755)` false, `(501, 0o775)` false, `(501, 0o757)` false,
   `(501, 0o644)` true. Each `assert!` carries the case in its message.
2. `#[cfg(unix)] fn a_folder_is_group_writable_only_for_wheel_or_admin()`: literal cases on `folder_safe` with
   `euid = 501`: `(501, 20, 0o755)` true, `(501, 20, 0o775)` false, `(501, 80, 0o775)` true,
   `(0, 80, 0o775)` true (the `/Applications` case), `(0, 0, 0o775)` true, `(501, 80, 0o777)` false,
   `(0, 0, 0o1777)` false (Voice keeps refusing sticky world-writable folders), `(502, 20, 0o755)` false,
   `(0, 20, 0o755)` true.
3. Change `a_root_folder_owned_by_the_person_is_lasting` (macOS only): today its last lines set the scratch
   `Applications` folder to `0o775` and assert it is still lasting. The scratch folder's group is the person's
   own group (`staff` on a Mac), so that case must now be **refused**. Replace those lines with: read the
   folder's gid (`std::os::unix::fs::MetadataExt`), `assert!(!matches!(gid, 0 | 80))` as a precondition, set
   `0o775`, assert `lasting(...)` is `Err(Error::Invalid(message)) if message == CLI_PLACE`, then set it back to
   `0o755`. Add a comment saying why a group-writable folder is refused here and `/Applications` is not.
4. Extend `a_folder_others_can_write_is_refused`: also set the command file to `0o664` (group write only)
   and assert it is refused with `CLI_PLACE`, restoring `0o644` after, in the style of the existing `0o646` case.

Do not touch any other test. Do not weaken any existing assertion.

## Run

From the repo root (`cargo` from stable Rust):

```
cargo fmt --all
cargo test --locked -p voice-core cli_command
cargo test --locked -p voice-api radio
cargo clippy --locked -p voice-core -p voice-api --all-targets -- -D warnings
```

Report each command's exit code. Do not run `scripts/ci-local.sh`; the reviewer runs the full gate.

## Never

- No new dependency. No file outside the two named.
- No real person's name, handle or path in any file (this repo is public; `AGENTS.md`). Use the literal uids
  above, not a real account.
- Don't commit; leave the changes in the working tree.

## Skills

Test-first: `~/.codex/skills/tdd-skill/SKILL.md` (Claude: `zizi-skills:tdd-skill`). Security standards:
`~/.codex/skills/security-and-hardening/SKILL.md` (Claude: `zizi-skills:security-and-hardening`).
