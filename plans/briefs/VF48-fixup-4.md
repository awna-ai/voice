# VF48 fix-up 4 brief: the Rust record check takes the spelling, and `record` resolves the written path

Card: VF48. Lane V, runner V1. You are Codex; implement this in the worktree you were started in, on top of
`HEAD`. Read `AGENTS.md` and the four earlier VF48 briefs in `plans/briefs/` first. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`. Leave every change uncommitted (your sandbox can't write git metadata);
Claude commits. Scratch only under a fresh `mktemp -d`, removed when done.

The repeat Verifier (Opus) failed fix-up 2 (`465b4e37`). Fix exactly these two.

## Must-fix

1. **The Rust record check refuses the new key.** `builder/ingest/src/mine.rs` now writes `fence_folder` on
   every mined record, but `check_record` (about line 642, the writer's exact-shape check) refuses any key it
   does not list, so `mine::tests::mined_fixture_records_pass_check_record` is red ("a history record has an
   unexpected shape"). `builder/api/src/sending.rs` (`fold`, about line 235) calls `check_record(...)?` on every
   delivered record, so a history sent from another computer fails the whole delivery. Fix: `check_record`
   accepts an optional `fence_folder` (a string matching `-[a-z0-9-]*`, the same rule Python applies; anything
   else is refused) on the record kinds mine writes it on, in its place in the key order mine writes. Accept
   `fence_folder_resolved` only where a live record can carry it, with the same rule, if `check_record` sees
   live records at all (read its callers to decide; say which in your report). Tests: the existing guard test
   green; a new case that a record whose `fence_folder` is not a string, or fails the pattern, is refused;
   `sending.rs`'s tests (find them under `builder/api`) still pass, plus a case that delivers a record carrying
   `fence_folder` and is accepted.
2. **`record` resolves the lexically normalised path, so `..` after a link names the wrong folder.**
   `scripts/crew_decisions.py` `cmd_record` (about lines 2093 to 2098) passes `lexical_path(...)` (made with
   `os.path.abspath`) to `path_compartment` and `safe_resolve`, and `abspath` folds `..` away before the link is
   followed. Repro (temp HOME, a map whose `work` folder is `~/projects/work`): `~/projects/work/link ->
   ~/elsewhere/a`; `record --work ~/projects/work/link/../secret/y.md` stores both spellings as
   `…-projects-work-secret-y-md` with `compartment: "work"`, though the file is `~/elsewhere/secret/y.md`; add
   the fence `~/elsewhere/secret` and `similar --compartment all` serves it. Fix: resolve
   `absolute_path(values["work"], cwd_of(args))` (absolute, not abspath'd, so `walk_resolve` follows the link
   before `..`) for both `path_compartment` and `safe_resolve`, with `"/"` as their cwd as now. `fence_folder`
   stays the lexical spelling (`lexical_path`, unchanged, which keeps the Rust parity). The relative-`--cwd`
   case of fix-up 2 must stay fixed (`absolute_path` already makes it absolute). Do not change
   `absolute_path`, `lexical_path`, `safe_resolve` or `walk_resolve`.

## DECISIONS

In the VF48 entry of `.ai/DECISIONS.md`, add a short clause that the resolved spelling follows links before
`..`, as the filesystem does. Keep the entry's wrap.

## Tests first (red on `HEAD`, on their assertions), then green

Rust: item 1's cases above. Python, class `Compartments` in `tests/crew_decisions_test.py`, links in the test's
own temp home: item 2's repro through the CLI (the stored `fence_folder_resolved` is the spelling of
`<home>/elsewhere/secret/y.md`, the compartment is not `work`, and with the later fence `similar` serves `[]`);
the fix-up 2 relative-`--cwd` test still passes.

Mutations (scratch copy, `cp` back, `cmp`; never `git checkout --`): `check_record` refuses `fence_folder`
again; `check_record` accepts any `fence_folder` value; `cmd_record` resolves `lexical_path` again. Each must
turn its test red.

## Run, at most 2 heavy processes at a time

`python3 -m unittest tests.crew_decisions_test` and the other suites under `tests/` that import
`crew_decisions`; `cargo fmt --check`; `VOICE_REQUIRE_PARITY=1 cargo test --locked -p voice-ingest` (the whole
crate: lib and integration tests, with the env and features `scripts/ci-local.sh` gives it); the `voice-api`
crate's tests; the cli record tests; `cargo clippy --all-targets --locked -- -D warnings`. End with a short
report: red reasons, mutation results, totals.
