# VF25 brief: Voice refuses a linked `voice` beside the app before it runs Radio

Lane V, runner V1, 2026-11-17. Card `1Rc8LTXq`. Filed from VF23's Verifier (a note: defence in depth, low).
Stacked on `lane-v1/VF23` (same file); it lands after VF23.

## Why

`builder/core/src/cli_command.rs` `shipped_cli` canonicalizes the running program and, when it isn't named `voice`
(the app), returns `<that canonical folder>/voice` without resolving that last part. `lasting` canonicalizes the path
and checks the target and the folders above the *target*. `shipped_lasting_cli` and `shipped_cli_word` then use the
unresolved path, and `builder/api/src/radio.rs` runs the `radio` beside it. So if `voice` in the bundle were a link to
a lasting copy elsewhere, the check passes on the target while Voice writes, hands Radio, and runs things from a
folder it never checked. Radio opens its handler with `O_NOFOLLOW` and requires a regular file
(`crates/radio/src/install.rs` `validate_handler`). Today a *folder* named `voice` in the command's place also passes
`lasting`.

## What to change

Only `builder/core/src/cli_command.rs` and `.ai/SECURITY.md`.

1. In `lasting`, before `path.canonicalize()`, read `std::fs::symlink_metadata(path)`. Map an error to
   `Error::Io { path: path.to_path_buf(), source }` exactly as the `canonicalize` error is mapped today (so
   `a_missing_command_is_an_io_error` stays true). If `!metadata.file_type().is_file()` (a link, a folder, anything
   else), return `Err(Error::Invalid(CLI_PLACE.into()))`. Everything after stays as it is. Only the last part of the
   path is checked this way: folders above it may be links (`/var` is a link to `/private/var` on a Mac), and
   `canonicalize` plus the folder walk already cover them.
2. `.ai/SECURITY.md`, two places, keeping the paragraph's wrapping near 100 characters:
   - The paragraph "A release build writes it only from a place that lasts": after "not under `/Volumes` or an
     `AppTranslocation` folder," add "the command must be a plain file (not a link or a folder),". Read the sentence
     after the edit and make sure it still parses.
   - The Radio row ("Voice installing Radio and talking to it"): in "(a release build refuses a copy on a mounted disk,
     a translocated one, one outside …", insert "a linked one," after "a translocated one,".

## Tests first (red, then green)

In the existing `#[cfg(test)] mod tests` of `cli_command.rs`, in its style:

```rust
// macOS has a per-user temp folder; Linux's /tmp is writable by others and refused.
#[cfg(target_os = "macos")]
#[test]
fn the_command_is_a_plain_file_not_a_link_or_a_folder() {
    let root = Scratch::new();
    let real = command_at(root.path(), "real/voice");
    assert!(lasting(&real, &[root.path()]).is_ok());
    let app = root.path().join("app");
    fs::create_dir(&app).unwrap();
    let linked = app.join("voice");
    std::os::unix::fs::symlink(&real, &linked).unwrap();
    assert!(matches!(
        lasting(&linked, &[root.path()]),
        Err(Error::Invalid(message)) if message == CLI_PLACE
    ));
    let folder = root.path().join("folder/voice");
    fs::create_dir_all(&folder).unwrap();
    assert!(matches!(
        lasting(&folder, &[root.path()]),
        Err(Error::Invalid(message)) if message == CLI_PLACE
    ));
}
```

Run it before the change and see it fail on an assertion (the link and the folder both pass today). Then make the
change and see it pass. `a_missing_command_is_an_io_error` must stay green: a missing path is still an `Io` error.
Do not touch any other test or weaken any assertion.

## Run

```
cargo fmt --all
cargo test --locked -p voice-core cli_command
cargo clippy --locked -p voice-core --all-targets -- -D warnings
```

Report each exit code. Don't commit. No other file, no new dependency, no real person's name, handle or path.

## Skills

`~/.codex/skills/tdd-skill/SKILL.md` (Claude: `zizi-skills:tdd-skill`), `~/.codex/skills/security-and-hardening/SKILL.md`
(Claude: `zizi-skills:security-and-hardening`).
