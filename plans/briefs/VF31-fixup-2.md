# VF31 fix-up 2: the repeat Verifier's findings

Read `plans/briefs/VF31-fence-link-refusal.md` and `plans/briefs/VF31-fixup-1.md` first; everything
there still holds except where this says otherwise. The code is `refuse_fence_links` and
`post_fence_link_todos` in `scripts/crew_decisions.py` (reuse `walk_resolve`, `absolute_path`,
`under`, `fold` and `FenceRoots` from the same file; add no new resolver). The tests are class
`FenceLinks` in `tests/crew_decisions_test.py`. Skills as before
(`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`). Public repo, invented fixtures only. **Commit nothing.**

Test first for each fix (red on the assertion, then green), and a mutation per fix (copy aside,
mutate, run, restore with `cp`, `cmp`).

## 1. A fence reached through a link is never probed inside another fence (must-fix)

Today the nested check compares spellings only, so `fenced: ["~/real", "~/alias/inner"]` with
`~/alias -> ~/real` passes it, and the `lstat` of `~/alias/inner` reads `~/real/inner`'s metadata
(killed under the tripwire). The same holds for `["~/secret", "~/alias/inner"]` with both links
pointing at `~/real`, and for an absolute spelling through `/private/...` of a fence under one
spelled through `/tmp` or `/var` (macOS links both into `/private`).

Rewrite the part of `refuse_fence_links` after the `..` check as three steps, in this order:

1. **Resolve each fence's parent, guarded.** Let `L_i = Path(os.path.abspath(absolute_path(value_i, Path.cwd())))`
   (the lexical form, as `lexical_fences` builds it). Visit the fences sorted by `len(L_i.parts)`,
   shortest first, ties in config order. For each one, call
   `walk_resolve(absolute_path(value_i, Path.cwd()).parent, guards)` where `guards` is a `FenceRoots`
   of every other fence's `L_j` plus every resolved form `R_j` found so far in this step. If the
   walk returns `blocked`, raise
   `CompartmentConfigError(f"compartments: the fenced folder {value_i} is reached through another fenced folder; fence only that folder")`.
   Otherwise `R_i = parent / L_i.name`, and remember whether the walk went through a link
   (`linked_i` is true when `parent` and `L_i.parent` differ by `fold` of their parts).
2. **Refuse nesting by either spelling**, still before any fence `lstat`: for every ordered pair
   `i != j` (skip exact duplicate values, as now), if `L_i` or `R_i` is under `L_j` or `R_j`
   (`under`, more parts than the outer), raise the existing message
   `the fenced folder {value_i} is inside the fenced folder {value_j}; fence only {value_j}`,
   first such pair in config order. This replaces the spelling-only loop.
3. **The one `lstat` per fence, of `R_i`, in two rounds.** Round 1: every fence with
   `linked_i` false, in config order. If round 1 found a link, raise the `FenceLinkError` now:
   round 2 never runs. Round 2: the fences with `linked_i` true, in config order. Handle errors,
   the dedupe by `(st_dev, st_ino)`, `links` (the `Path(value).expanduser()` spelling, as now) and
   the message clauses exactly as now.

No other filesystem access: no `stat`, `readlink` beyond what `walk_resolve` already does on
the parents, no `scandir`, no `realpath`/`resolve`.

Tests (each red today, then green):
- `["~/real", "~/alias/inner"]` with `~/alias -> ~/real`: refused with "reached through another
  fenced folder"; with `os.lstat` wrapped, no call on any path under `~/real` other than `~/real`
  itself (the walk's check of `~/real` happens before its `lstat`, so expect none at all under
  `~/real`; assert what you observe and say which). Through the CLI under
  `tripwire_crew(..., [home/"real"], ..., allow_root_metadata=True)`, `where` exits 2, not -9.
- `["~/secret", "~/alias/inner"]` (both links to `~/real`): refused as a link (`~/secret`), and no
  `lstat` of any path under `~/real` deeper than `~/real` itself; the same CLI tripwire run exits 2.
- The `/private` case: build the two spellings from the test's own temp folder (take
  `home.resolve()`; when it differs from `home` as a string, fence `~/real` and
  `str(home.resolve() / "real/inner")`, else `skipTest`). Refused with "reached through another
  fenced folder" (the walk meets `~/real`'s resolved form); tripwire exits 2.
- `test_real_missing_and_linked_parent_load` and every existing `FenceLinks` test stay green
  (a fence under a linked parent folder that is not fenced still loads).

## 2. The xattr subtest works on macOS (must-fix)

`test_tripwire_root_metadata_allowance_still_kills_reads` calls `os.getxattr`, which CPython on
macOS doesn't have, so the child fails with `AttributeError` (exit 1) instead of -9. Replace that
case's operation with a `ctypes` call to libc's `getxattr` (macOS signature:
`getxattr(path, name, value, size, position, options)`), for example:
`import ctypes, ctypes.util; libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True); libc.getxattr(<path bytes>, b"invented", None, 0, 0, 0)`.
The child must not catch the signal; expected -9. Add one more case that proves the call itself
works outside a fence: `getxattr` on a file outside the fence returns -1 with `ENOATTR` (93) and
the child exits 0 (print nothing; `assert ctypes.get_errno() == 93`).

## 3. The TODO dedupe compares the whole item text (should-fix)

A link whose path contains `x the fenced folder <home>/secret is a link.` still matches the
substring marker. Build the full item text once per link (as `add` sends it), and skip the
`add` only when an open item's text, whitespace-normalized as now, **equals** that text,
whitespace-normalized. Test: seed an open item whose text is the full text for `~/secret`
with extra words before it; the refusal still adds its own item. `test_todo_marker_ends_at_sentence`
and the dedupe tests stay green.

## Done when

- With Python 3.12 (`$(pyenv prefix 3.12)/bin/python3` if `python3` is older):
  `tests/crew_decisions_test.py`, `tests/crew_documents_test.py` and `tests/crew_hardening_test.py`
  pass. The kernel tripwire tests may skip in your sandbox; say which ran and which skipped.
- `git diff --stat` shows only `scripts/crew_decisions.py` and `tests/crew_decisions_test.py`.
- Your final message lists each new test with its red run and each mutation's result.
