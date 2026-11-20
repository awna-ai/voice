# VF31 fix-up 4: the follow rule holds however a fence is spelled

Fix-up 3 and 3b (`9d9f0ac`) still hold except where this says otherwise. The security review on
`9d9f0ac` found that the rule keys on how a fence is *written*: a fence spelled outside home that
leads into home (`/private<home>/hop/f` on macOS, the firmlink spelling
`/System/Volumes/Data/<home>/...`, or a link outside home pointing into home) walks its whole parent
freely, so `~/hop` is read with no `follow`, and R3 comes back (`lstat` under the fenced link's
target). **Commit nothing.** Skills as in fix-up 3. Invented fixtures only.

## 1. One predicate, decided by where the walk is, not by the spelling (high)

Replace the free-prefix split of 3b (the `start`, `outside_base` and `starts` machinery) with one
rule. Every fence's parent walk is `walk_resolve(lexical[i].parent, guards, may_follow=pred)`, from
`/`, guards as today. `pred(candidate)` (called after the `lstat` that shows a link, before
`readlink`, as today):

- **Inside home** means: `candidate` is strictly below `home` or `home_resolved` by `under` (not
  equal), **or** some ancestor `a` of `candidate` (from `candidate.parent` up to `/`) has
  `os.stat(a)`'s `(st_dev, st_ino)` equal to home's (`os.stat(home_resolved)`, taken once, beside the
  home walk; a failure there is the existing fail-closed "cannot check" error). The identity check
  runs only when the spelling check says "not inside"; an `OSError` from it counts as inside home
  (fail closed). These ancestors are folders the walk has already passed, never a fence (the fence
  check runs first) and never the link itself.
- A link inside home is followed only when it sits at or below a follow folder (spellings as in
  fix-up 3), and then `linked[i] = True`.
- Any other link (on home's own path, or outside home) is followed freely. This drops the
  "outside-home common folder" clause: links outside home never needed it, so the review's L4 (a
  second outside fence making `/tmp` refused) goes too.
- Keep `base = commonpath(...)` inside the `try` only if a test still needs it; otherwise remove it
  and adjust `test_commonpath_errors_fail_closed_naming_first_fence` to patch the home walk's failure
  instead. Say which you did.

## 2. Follow folders match exactly (low)

`fold` (NFC plus casefold) is the safe direction for fences but the loose one for `follow`:
`follow: ["~/strasse"]` lets `~/straße/l` be followed. For follow matching only, compare
`unicodedata.normalize("NFC", part)` parts exactly, with no casefold (a new tiny helper beside
`under`). A different case on a case-insensitive volume is then refused: that fails closed, and the
message already names the fix.

## 3. No control characters in the refusal (low)

The link path shown in the unfollowed-link message comes from link targets. Before formatting, replace
every character for which `unicodedata.category(ch)[0] == "C"` with `?` in `shown` (and in the
`values[i]` printed beside it).

## 4. Tests (class `FenceLinks`)

- R4 through `str(<resolved home>/hop/f)` when that differs from the `~` spelling (skip otherwise),
  and through a link outside home pointing at home (`<tmp>/out/lnk -> <home>`, fence
  `<tmp>/out/lnk/hop/f`): both refused naming the hop link, no `readlink` of it.
- R3 with `["~/sealed", str(<resolved home>/alias/inner/deep)]` (`~/sealed`, `~/alias` -> `~/real`):
  refused, no `readlink` of alias, no `lstat` under `~/real` either spelling.
- The identity check: patch `under`-based spelling to miss (e.g. a fence spelled through a second
  invented link outside home that points at home), and prove it is the stat identity that refuses.
  One mutation removing the identity clause must go red.
- `/tmp`-style system link with two outside fences: `[<tmp>/a/f, <tmp2>/b/g]` with no `follow`
  loads (L4).
- `follow: ["~/strasse"]`, `fenced: ["~/straße/l/f"]`, `~/straße/l` a link: refused (skip if the
  volume folds the two names into one folder).
- A link target holding an ESC character: the message holds `?` and no ESC.
- Every existing `FenceLinks` test stays green, adjusted only where section 1 changes a message, and
  say which.

Run with Python 3.12 first on PATH, one process at a time: `python3 tests/crew_decisions_test.py
FenceLinks`, `python3 tests/crew_documents_test.py`, `python3 tests/crew_hardening_test.py`. One
mutation per section (copy aside, mutate, run, restore with `cp`, `cmp`). Summarise the diff, reds
and mutations.
