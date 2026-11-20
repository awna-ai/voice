# VF31 fix-up 7: the backstop matches every spelling, home's own path matches exactly, one test needs `follow`

Fix-up 6 (`919ef9a`) still holds except where this says otherwise. Read `plans/briefs/VF31-fixup-6.md` (with its
three amendments) first. The code is `refuse_fence_links` in `scripts/crew_decisions.py`; the tests are in
`tests/crew_decisions_test.py`. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`. Public repo: invented
fixtures only. **Commit nothing.** Leave `.ai/DECISIONS.md` alone.

## 1. The backstop compares the way the walk guards do (Verifier must-fix)

On macOS `/System/Volumes/Data/<path>` is the same folder as `<path>` (a firmlink). The backstop compares with
`under` (parts only), so a fence path that goes through a link into the `/System/Volumes/Data` spelling still
**loads** after looks inside a fence. The walk guards use `matches_fence`, which also matches the encoded spelling
as a substring, and catch it.

- Change the backstop's test to
  `any(matches_fence(path, FenceRoots([resolved[j], lexical[j]])) for path in looked)` for each fence `j` in config
  order. Nothing else changes (same message, same place, after the rounds).
- Test (skip unless `Path("/System/Volumes/Data").is_dir()` and `os.path.samefile("/System/Volumes/Data" + str(D.resolve()), D.resolve())`;
  `D` a fresh `tempfile.mkdtemp()` outside home): `follow: [str(D.resolve())]`,
  `fenced: [str(D / "fol/al/x"), str(D / "l1/l2/f1")]`, `D/l1/l2 -> D/real`,
  `D/fol/al -> "/System/Volumes/Data" + str(D.resolve() / "real/f1/sub")`, folders `D/fol`, `D/l1`,
  `D/real/f1/sub` present. Fix-up 6 loads it; now refused with the backstop message (the fence it names is
  whichever comes first in config order with a look at or inside it; assert the exact message for that fence),
  and not a `FenceLinkError`. Mutation: back to `under`: red.

## 2. Home's own path matches exactly (both reviews, low)

`home_parts` and the lookup in `may_follow` fold case (`fold`), so on a case-sensitive volume a different link
that only folds equal to a folder on home's path (`/HOME` beside `/home`) is followed without `follow`.

- Build `home_parts` and the lookup with `tuple(unicodedata.normalize("NFC", part) for part in path.parts)` (NFC,
  no casefold), the way `under_follow` compares. Nothing else changes; the `linked[i]` rule is unchanged.
- Test (skip unless `os.path.islink("/var")` and home's spelling starts with `/var/`, the macOS test home):
  `fenced: ["/VAR" + str(self.home)[4:] + "/f"]` (the same folder, `/var` spelled in capitals; the volume folds
  case so the walk's `lstat` of `/VAR` finds the `/var` link). With exact matching `/VAR` is not on home's own
  path, so the load is refused through the link `/VAR` (the unfollowed message). Mutation: back to `fold`:
  it loads, red.

## 3. One more test needs `follow` (fix-up 6's exemption rule)

`Compartments.test_fence_parent_link_and_link_hop_limit` fences `base / "alias" / "private"` through the link
`base / "alias"` outside home, which fix-up 6 now refuses without `follow`. It is exempt under fix-up 6's rule:
add `self.config["follow"] = [str(base)]` right after the `fenced` line. Nothing else changes.

## 4. Run

`PATH=~/.pyenv/versions/3.12.14/bin:$PATH python3 tests/crew_decisions_test.py FenceLinks` and
`... crew_decisions_test.py Compartments`; one mutation for sections 1 and 2 (copy aside, mutate, run the one test,
restore with `cp`, check with `cmp`). Skip the three full suites: the Claude reviewer runs them outside your
sandbox. Summarise the diff, each red you saw first, each mutation and its result, and the counts. If any other
test fails, stop and report.
