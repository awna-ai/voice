# VF35 fix-up 1: the Mac's other root aliases (`/.nofollow`, `/.resolve`, `/.vol`) are refused too

The brief `plans/briefs/VF35-data-volume-spelling.md` (built at `a241efe`) still holds except where this says
otherwise. Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`. This is a public repo: use invented fixtures only. **Commit nothing.** Leave `.ai/**` alone.

## The finding (security review of `a544282`, medium; checked on this Mac)

macOS also reaches every folder through root entries whose names start with a dot: `/.nofollow/Users`,
`/.resolve/1/Users` and `/System/Volumes/Data/Users` have the same device and inode as `/Users`. `under(path,
DATA_VOLUME)` only catches the third. So a fence written `"/.nofollow" + <path>` (or
`"/.nofollow/System/Volumes/Data" + <path>`) loads, and afterwards the plain `<path>` reads as not fenced. The same
holds for a `follow` value, and for a fence whose link resolves into `/.nofollow/...`.

## The decision (do not change it)

Refuse any absolute path whose first folder after `/` starts with `.`, as well as the data-volume spelling, in the
same three places as before (the follow loop, the fenced loop, and right after `resolved[i]`). No real config
fences or follows a folder directly under `/` whose name starts with a dot. This fails closed and needs no list of
alias names.

- Replace the three `under(..., DATA_VOLUME)` tests with one helper next to `DATA_VOLUME`:

  ```python
  def mac_alias(path):
      """The start of path that a Mac may use as a second name for other folders, or None."""
      if len(path.parts) > 1 and path.parts[1].startswith("."):
          return "/" + path.parts[1]
      if under(path, DATA_VOLUME):
          return str(DATA_VOLUME)
      return None
  ```

  The dot rule comes first, so `/.nofollow/System/Volumes/Data/...` reports `/.nofollow`.
- The messages, with `prefix` the helper's result:
  - follow: `f"compartments: the follow folder {shown(value)} starts with {shown(prefix)}, which a Mac can use as a second name for other folders; write it without {shown(prefix)}"`
  - fenced, written: `f"compartments: the fenced folder {shown(value)} starts with {shown(prefix)}, which a Mac can use as a second name for other folders; write it without {shown(prefix)}"`
  - fenced, resolved: `f"compartments: the fenced folder {shown(values[i])} is reached through a link into {shown(prefix)}, which a Mac can use as a second name for other folders; write the fence by its path without {shown(prefix)}"`
- Nothing else changes.

## Tests

- Update the expected messages in the VF35 tests to the new wording (prefix `/System/Volumes/Data`).
- In `test_data_volume_spelling_refused_before_any_look`, add subtests (same assertions: exact message, not a
  `FenceLinkError`, and no probe whose path is under `/System/Volumes/Data` **or** whose first part after `/` starts
  with `.`):
  - fenced `"/.nofollow" + str(self.home / "real/inner")`, prefix `/.nofollow`;
  - fenced `"/.resolve/1" + str(self.home / "real/inner")`, prefix `/.resolve`;
  - fenced `"/.nofollow/System/Volumes/Data" + str(self.home / "real/inner")`, prefix `/.nofollow`;
  - follow `"/.vol"`, with `fenced: ["~/real/inner"]`, prefix `/.vol`.
- New test `test_fence_through_link_into_nofollow_refused`: skip unless
  `os.path.samefile("/.nofollow" + str(D.resolve()), D.resolve())` (where `D` is a fresh `tempfile.mkdtemp()`, cleaned up).
  `D/real/a/f` present, `D/L -> "/.nofollow" + str(D.resolve() / "real")`, `follow: [str(D.resolve())]`,
  `fenced: [str(D / "L/a/f")]`. Assert the exact resolved message with prefix `/.nofollow`, not a `FenceLinkError`.
- Mutations (copy aside, mutate, run the one test, restore with `cp`, confirm with `cmp`):
  - make the helper ignore the dot rule: the new subtests and the new test go red;
  - make the helper check `DATA_VOLUME` first: the `/.nofollow/System/Volumes/Data` subtest goes red.

## Run

`PATH=~/.pyenv/versions/3.12.14/bin:$PATH python3 tests/crew_decisions_test.py FenceLinks`, then
`... Compartments`. Skip the full suites. If any other test fails, stop and report it. Summarise the diff, each
red you saw first, each mutation and its result, and the counts.
