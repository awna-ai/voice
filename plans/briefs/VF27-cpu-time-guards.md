# VF27: the crew routing time guards measure CPU time, so a busy Mac doesn't fail them

Branch `lane-v1/VF27`, off `origin/main`. Test-only. Small.

## Why

Two linear-time guards in `tests/crew_decisions_test.py` time the wall clock: `signal.setitimer(signal.ITIMER_REAL, …)`
with a `SIGALRM` handler, and `time.perf_counter()` for `elapsed`. On a loaded Mac they fail with no code change: on
`main`, at load average 22, `test_hub_distinct_megabyte_names_with_two_fences` failed its 4-second deadline after 2.1 s
of CPU time. The guards are there to catch a quadratic matcher, so they should count the CPU time the process spends,
which other processes on the machine don't inflate.

## Steps

1. **`Compartments.test_hub_routing_megabyte_strings_are_linear`** (around line 1576) and
   **`Compartments.test_hub_distinct_megabyte_names_with_two_fences`** (around line 1949), and nothing else:
   - `signal.SIGALRM` becomes `signal.SIGPROF` (both the `signal.signal(...)` that installs the handler and the one in
     `finally` that restores it).
   - Every `signal.ITIMER_REAL` becomes `signal.ITIMER_PROF` (the arming call, the disarming call inside the loop, and
     the one in `finally`).
   - Both `time.perf_counter()` calls in each test become `time.process_time()`.
   - Keep the bounds (2 and 4), the messages, the structure and the assertions exactly as they are, except the two
     failure messages say CPU time: `"hub routing exceeded 2 seconds of CPU time"` and
     `"distinct hub routing exceeded 4 seconds of CPU time"`.
2. Leave `tests/crew_extra_sources_test.py`'s `ITIMER_REAL` alone: it checks that `crew_mail.budget()`, product code
   with a wall-clock budget, leaves an earlier `ITIMER_REAL` timer running. That is behaviour, not a speed guard.
   Leave `scripts/crew_mail.py` alone.
3. Run, with at most 2 workers and nothing else in parallel:
   - `python3 -m unittest tests.crew_decisions_test.Compartments.test_hub_routing_megabyte_strings_are_linear tests.crew_decisions_test.Compartments.test_hub_distinct_megabyte_names_with_two_fences`
   - `python3 -m unittest tests.crew_decisions_test`

## Rules

- Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`.
- Touch only `tests/crew_decisions_test.py`. Public repo: no real person's data. Commit nothing.
- Summarise: the diff, and each run's result line (`Ran N tests …`, `OK` or the failure).

## Fix-up 1: the distinct-names bound is 15 seconds of CPU time

CPU time alone doesn't fix the distinct-names guard. Measured on the loaded Mac with the steps above in place,
`hub_compartment` on the 30,000 distinct names takes 5.1 to 5.3 s of CPU time (`process_time()`), because the busy
scheduler runs it on the slower efficiency cores, and the work is linear but costs about 0.15 ms per distinct name. The
megabyte-strings guard takes 0.5 s and keeps its 2-second bound.

A real regression is far bigger: folding the whole prefix before each match (what `3df19b5` replaced with the bounded
window) costs minutes on these inputs. So a 15-second CPU bound keeps the guard and leaves room for slow cores.

1. In `test_hub_distinct_megabyte_names_with_two_fences` only: `signal.setitimer(signal.ITIMER_PROF, 4)` becomes
   `signal.setitimer(signal.ITIMER_PROF, 15)`, `self.assertLess(elapsed, 4)` becomes `self.assertLess(elapsed, 15)`, and
   the message becomes `"distinct hub routing exceeded 15 seconds of CPU time"`. Add one comment line above the
   `setitimer` call: `# CPU time, not wall time: a busy Mac runs this on slow cores; a quadratic matcher takes minutes.`
2. Nothing else changes. Run the two focused tests and the module, as in step 3 above.
