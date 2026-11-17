# VF23 fix-up 1: pin which rule the command gets

Lane V, runner V1, 2026-11-17. Follows `plans/briefs/VF23-group-writable-folder.md` (already built at `c5b5679`).

## Why

The Verifier found that no test pins the wiring in `lasting` (`builder/core/src/cli_command.rs`): replacing the
index-0 call `command_safe(..)` with `folder_safe(metadata.uid(), metadata.gid(), metadata.mode(), euid)`, or with
`command_safe(..) || folder_safe(..)`, leaves every test green, because the scratch file's group is `staff` and
`0o664` is refused by both functions. Then a `voice` owned `user:admin` with mode `0775` would pass Voice while Radio
refuses it, which is this card's bug class.

## What to change

Only `builder/core/src/cli_command.rs` and `.ai/SECURITY.md`.

1. Add one private `#[cfg(unix)]` pure function next to `command_safe` and `folder_safe`:

   ```rust
   /// One path on the way up from the command: the command itself gets the command's rule, every folder above it
   /// the folder rule.
   #[cfg(unix)]
   fn place_safe(is_command: bool, owner: u32, group: u32, mode: u32, euid: u32) -> bool
   ```

   Body: `if is_command { command_safe(owner, mode, euid) } else { folder_safe(owner, group, mode, euid) }`.

2. In `lasting`'s loop, replace the `let safe = if index == 0 { … } else { … };` block with
   `place_safe(index == 0, metadata.uid(), metadata.gid(), metadata.mode(), euid)`; refuse with `CLI_PLACE` when it
   is false, as now. Nothing else in `lasting` changes.

3. Test first, in the existing `mod tests`, before step 1 and 2 (it won't compile until `place_safe` exists, so
   write it, then add `place_safe` with a body of `false` to see it fail on its assertion, then the real body):

   ```rust
   #[cfg(unix)]
   #[test]
   fn the_command_gets_the_command_rule_and_folders_the_folder_rule() {
       let euid = 501;
       assert!(!place_safe(true, 501, 80, 0o775, euid), "command (501, 80, 0o775)");
       assert!(place_safe(false, 501, 80, 0o775, euid), "folder (501, 80, 0o775)");
       assert!(place_safe(true, 501, 20, 0o755, euid), "command (501, 20, 0o755)");
       assert!(!place_safe(false, 501, 20, 0o775, euid), "folder (501, 20, 0o775)");
   }
   ```

4. `.ai/SECURITY.md`, the paragraph that says "never by others (Radio's own rule for a handler)": change it to
   "never by others (Radio's own rule for a handler, except that Voice also refuses a world-writable folder with the
   sticky bit, which Radio allows)". Keep the paragraph's line wrapping near 100 characters.

## Run

```
cargo fmt --all
cargo test --locked -p voice-core cli_command
cargo clippy --locked -p voice-core --all-targets -- -D warnings
```

Report each exit code. Don't commit. No other file; no real person's name, handle or path.

## Skills

`~/.codex/skills/tdd-skill/SKILL.md` (Claude: `zizi-skills:tdd-skill`).
