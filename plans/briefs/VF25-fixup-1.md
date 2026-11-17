# VF25 fix-up 1: callers use the path `lasting` checked

Lane V, runner V1, 2026-11-17. Follows `plans/briefs/VF25-linked-command.md` (built at `8f4e22e`).

## Why

The Verifier found a race: `lasting` (`builder/core/src/cli_command.rs`) reads `symlink_metadata(path)` and then
`path.canonicalize()`, two separate lookups. Someone who can write the folder Voice runs from can flip `voice`
between a file and a link to a lasting copy; when the first lookup sees the file and the second the link, the folder
walk checks the lasting copy's folders while callers still use the unchecked `path` (she hit it 361 times in 200,000
tries). The close: `lasting` returns the canonical path it checked, and every caller uses that path, never the one it
was given.

## What to change

Only `builder/core/src/cli_command.rs` and `.ai/SECURITY.md`.

1. `fn lasting(path: &Path, roots: &[&Path]) -> Result<PathBuf>`: keep every check exactly as it is and return
   `Ok(canonical)` at the end. Keep the `symlink_metadata` plain-file check before `canonicalize`.
2. `fn lasting_at_home(path: &Path, home: &Path) -> Result<PathBuf>`: return what `lasting` returns.
3. `shipped_lasting_cli`: in a release build return `lasting_at_home(&path, home)` (the canonical path); in a
   development build return `path` as today. Write it so the `cfg!(debug_assertions)` branch is the same shape as now.
4. `word_from(path, home, lasting_only)`: when `lasting_only`, call `cli_word` on the path `lasting_at_home` returned;
   otherwise on `path` as today.
5. Update the doc comments of `shipped_cli_word` and `shipped_lasting_cli` by one clause each: in a release build the
   word or path is the canonical one the rule checked.
6. `.ai/SECURITY.md`, the paragraph "A release build writes it only from a place that lasts": after
   "(`builder/core/src/cli_command.rs` tests)." add the sentence "The command Voice then writes, hands Radio or runs
   beside is the canonical path the rule checked, never the path it was given." Keep the wrapping at 100 characters
   or less.

## Tests

Existing tests that compare `word_from(&command, home, true)` with `cli_word(&command, home)` now get the canonical
path from `word_from` (the scratch folder is under `/var`, a link to `/private/var` on a Mac). Change only their
expected side to `cli_word(&command.canonicalize().unwrap(), home)`; do not change what they assert otherwise.
Calls like `assert!(lasting(...).is_ok())` stay as they are.

Add one test first and see it fail (it won't compile until `lasting` returns a path; that compile failure is the
red here, so say so in your report):

```rust
#[cfg(target_os = "macos")]
#[test]
fn the_lasting_path_is_the_canonical_one_checked() {
    let root = Scratch::new();
    let command = command_at(root.path(), "Applications/Voice.app/Contents/MacOS/voice");
    let expected = command.canonicalize().unwrap();
    assert_ne!(command, expected, "the scratch path should go through the /var link");
    assert_eq!(lasting(&command, &[root.path()]).unwrap(), expected);
    assert_eq!(
        word_from(&command, root.path(), true).unwrap(),
        cli_word(&expected, root.path()).unwrap()
    );
}
```

## Run

```
cargo fmt --all
cargo test --locked -p voice-core cli_command
cargo test --locked -p voice-api radio
cargo clippy --locked -p voice-core -p voice-api --all-targets -- -D warnings
```

Use `CARGO_BUILD_JOBS=2`. Report each exit code (your sandbox may refuse the socket tests in `voice-api radio`; say
which). Don't commit. No other file, no real person's name, handle or path.

## Skills

`~/.codex/skills/tdd-skill/SKILL.md` (Claude: `zizi-skills:tdd-skill`).
