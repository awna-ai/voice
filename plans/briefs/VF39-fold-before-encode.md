# VF39 brief: a fence's encoded spelling folds non-ASCII letters before encoding

Card: VF39 (Persona Builder board). Lane V, runner V1. You are Codex; implement this in the worktree you were
started in. Read `AGENTS.md` first. Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.

## The defect (reproduced by Claude on `386e0fd`, Python 3.12.14, Mimi)

`matches_fence` in `scripts/crew_decisions.py` compares a look two ways: its folded path parts against each fence
root's folded parts, or each root's encoded spelling as a substring of the look's encoded spelling. The encoded
spellings are `fold(encode(path))`. `encode` turns every character that is not an ASCII letter or digit into `-`
**before** `fold` runs, so a non-ASCII letter that APFS opens as an ASCII one is lost:

- On APFS (case-insensitive), `~/ſecret` (U+017F, long s) opens `~/secret`, and `~/straße` opens `~/strasse`
  (measured on Mimi with `ls`). `fold` already maps them (`"ſ".casefold() == "s"`, `"ß".casefold() == "ss"`).
- A look `/System/Volumes/Data<home>/ſecret/file.txt` under the plain fence `~/secret`: the parts check fails
  (the look starts `/System/...`), and the encoded check fails too (`...-<home>--ecret-...` does not contain
  `...-<home>-secret`). `matches_fence` returns `False` (measured). The plain spelling `<home>/ſecret/f.txt`
  already returns `True` through the parts check.
- The Kelvin sign (U+212A) is **not** affected: `encode`'s NFC step already turns it into `K` (measured). Do not
  add code for it; one assertion that it stays fenced is enough.

## The fix (decided; do exactly this)

1. Add one helper next to `encode`:

   ```python
   def fence_encode(path):
       """encode(path), each non-ASCII character case-folded first: APFS opens ſ as s and ß as ss."""
   ```

   It expands `~` (as `encode` does), NFC-normalizes, replaces each non-ASCII character with its
   `str.casefold()` (ASCII characters stay as they are, so `project_name`'s home-prefix strip still works), then
   returns `encode(...)` of that string. Do **not** change `encode` itself: it mirrors how Claude Code names a
   project folder (`cwd_folder = encode(cwd)` and friends compare against real folder names) and must keep doing so.
2. Use `fence_encode` in exactly these four fence comparisons, and nowhere else:
   - `FenceRoots.__new__`: `fold(fence_encode(root))` in place of `fold(encode(root))`;
   - `matches_fence`: `encoded = fold(fence_encode(path))`;
   - `folder_compartment`'s first check: `fold(fence_encode(f)) in folded`;
   - `record_compartment`'s fenced check: `fold(project_name(fence_encode(f))) in fold(project)`.
   The compartment and hub routing comparisons (`declared_folders`, `hub["paths"]`, `hub["routes"]`) stay as they
   are: they are not fences, and widening them could move a record between compartments.
3. Folding first only ever makes a fence match more looks (fail-closed). Say so in one line in `.ai/DECISIONS.md`
   under a new `## 2026-11-20 — VF39: a fence's spelling folds before it is encoded` entry (3 to 6 lines: the
   defect, the helper, why `encode` is unchanged, why routing is unchanged, the Kelvin note).

## Tests first (red, then green), in `tests/crew_decisions_test.py`

Invented folders only. Run each new test against the unfixed code first and see it fail on its assertion.

- In `FenceLinks` (it has `self.home`, the fence `~/secret` → `real`, and `require_data_volume_spelling`), add
  `test_long_s_look_in_data_volume_spelling_is_fenced`: build a plain (non-link) fence folder in a fresh
  `tempfile.mkdtemp()` outside the fixture's links (as `data_volume_folder` does), call
  `require_data_volume_spelling` on it, and skip unless `os.path.samefile` says the `ſ` spelling opens the same
  folder. Then assert that `matches_fence` with that fence's `FenceRoots` returns `True` for the look in the
  `/System/Volumes/Data` spelling with `ſ` for an `s` in the fence's last name, and for the same look with `ß`
  where the fence name has `ss` (skip that subtest unless `samefile` agrees). Name the fence folder with an `s`
  (and one with `ss`), for example `secret-notes` and `glass-jar`.
- In the same class, a CLI test through `self.cli("where", "--cwd", <that look's folder>)` with the config's
  `fenced` set to that folder: exit code 4 (fenced), as the other `where` refusals in this file assert. Use the
  same skips.
- In `Compartments`, add `test_fence_spelled_with_long_s_matches_folder_and_record`: with `fenced` set to
  `["~/garden-club", "~/ſecret-notes"]`, `folder_compartment(c, encode("~/secret-notes"))` is `"fenced"` and
  `record_compartment(c, {"project": project_name(encode("~/secret-notes"))})` is `"fenced"`. These are pure
  string checks; no skip.
- One assertion that a Kelvin-sign look (`K` written as U+212A) under a fence with a `k` stays fenced through
  `matches_fence` (it passes before and after the fix; it pins the NFC step).
- Keep `test_folded_fence_comparisons_prepared_once` passing: the roots must still be encoded once, when
  `FenceRoots` is built. If it patches `encode`, make sure `fence_encode` calls the module's `encode` by name so
  the patch still sees calls.

Mutation check: after green, revert only the `matches_fence` line to `fold(encode(path))` and confirm the
data-volume test goes red; restore it. Do the same for the `folder_compartment` line and its test.

## Run

- `~/.pyenv/versions/3.12.14/bin/python3 tests/crew_decisions_test.py FenceLinks Compartments` (both green, with
  only the skips the file already had plus yours where the spelling does not open the same folder);
- the whole file once at the end with the same interpreter;
- `shellcheck` is not touched. Do not run `scripts/ci-local.sh` (Claude runs the gate).

## Never

- No real names, handles or paths of a real person in any file; invented folder names only.
- Do not change `encode`, the routing comparisons, or any refusal message.
- Do not touch files other than `scripts/crew_decisions.py`, `tests/crew_decisions_test.py` and `.ai/DECISIONS.md`.
- Do not commit; leave the changes in the worktree.
