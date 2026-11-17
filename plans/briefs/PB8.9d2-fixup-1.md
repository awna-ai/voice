# PB8.9d2 fix-up 1: the Verifier's findings

Lane V, runner V1, 2026-11-17. Same rules, skills and gate as `plans/briefs/PB8.9d2-add-another-computer-screens.md`.
Test first: each new test fails on today's code for the right reason. Don't commit; Claude reviews and commits.

## 1. Must-fix: the card is a dead end when Radio isn't running

`radioLinkAddress` is the card's first action, and `pairing::link_address` never starts Radio, so with Radio down the
person gets "Radio isn't running on this computer." and nothing else. Fix in `builder/api/src/pairing.rs`:

- Add `fn started_address(home: &Path, radio: &Radio) -> Result<dto::LinkAddress>` that runs `ready(home, radio)?`
  then `radio.link_address_list().map(addresses).map_err(radio_error)`.
- `radio_link_address()` (the app wrapper) calls `started_address(&home, &Radio::for_home(&home))` and maps through
  `named`, so the app starts Radio on its first click exactly as setting an address, inviting and joining already do.
  `pub fn link_address` (the CLI's `voice radio address`) stays as it is.
- Test in `builder/api/src/pairing/tests.rs`, with the existing fake-Radio support: `started_address` against a
  socket that isn't there runs the install path the way `set_address_in`'s test for a stopped Radio does (mirror that
  test; if that test shape can't reach `ready`'s install branch without a real Radio, test that a running fake answers
  `hello` then `link_address_list` in that order, as the set-address tests check request order).

## 2. The recovery sentence stays for the card's life

- Keep `recoveryFile` in its own ref, set from each invite answer only when non-null, never cleared by a later invite
  (a second invite answers `null`: Radio is "already in a fleet"). Show the warning from that ref.
- When an invite **fails** and its message contains the recovery sentence (the engine appends `text::recovery(path)`
  after a failure that came after the fleet was made), take the path out with a regular expression built from
  `recoverySentence` in `radio-words.ts` (export a `recoveryPath(message: string): string | null` there, escaping the
  template's fixed text) and set the same ref, so the warning outlives the next click that clears `error`.
- Update the test at `OtherComputers.test.ts` that now asserts the sentence goes away on *Make a new code*: it must
  stay. Add: an invite failure carrying the sentence, then a successful invite answering `null`, still shows the
  warning; `recoveryPath` returns `null` for a message without the sentence.

## 3. Words that don't promise what isn't built, and the full steps

Sending history comes in a later slice (PB8.9e); today linking only pairs the computers.

- Builder intro: `Link another computer to this one. For now this only links them: {{ name }} doesn't learn from the
  other computer yet. Both computers need to be on your VPN, and the other one needs {{ name }} first: there, choose
  Make a new assistant, then Open one you saved on GitHub.`
- Member intro: `Link this computer to the one that builds {{ name }}. For now this only links them: {{ name }}
  doesn't learn from this computer yet. Both computers need to be on your VPN.`
- Builder instruction under the code: `On the other computer, open {{ name }}'s Save step, choose Link this computer
  under Your other computers, paste this code, and choose Join.`

## 4. Accessibility and small guards

- The shown code's label: replace `<label for="join-code-shown">` with `<div id="join-code-label"
  class="form-label">Join code</div>` and give the `<code>` `aria-labelledby="join-code-label"`.
- Announce a copy: beside the Copy button, `<span v-if="copied" class="visually-hidden" role="status">Copied the join
  code.</span>`.
- `copy()` sets `action = 'copy'` (add it to the union) while the write runs, so nothing else starts meanwhile and
  *Copied* can't sit beside a newer code.
- A saved address change clears the shown code (`invitation`) and `copied`: a code carries the address it was made
  with. The recovery ref stays.
- Render the `btn-list` wrapper only when it has a button (`v-if="hasAddress"` on the wrapper, not the button).

## 5. Tests the Verifier's mutations showed missing

In `OtherComputers.test.ts`, each must fail when its guard is removed (mutation-check each, `cp` backup and `cmp`):

- For each of `radioLinkAddress`, `radioSetLinkAddress`, `radioInvite` and `radioJoin`: a plain `{ message }`
  failure shows that message, and an `Error('private detail')` shows that action's exact fallback sentence and never
  `private detail`.
- With a pending (`deferred`) action, *Copy*, *Make a new code*, *Change it*, the address field, *Save address*, the
  name and code fields and *Join* each carry `disabled`; and calling the component's handlers directly while busy
  (as the existing busy test does) calls no backend method. Make the existing busy test reach each guard: give it an
  invitation for `copy`, a member role with filled fields and an address for `join`, and a filled field for
  `saveAddress`, so the only reason each returns early is `busy`.
- Builder role, after *Add another computer* succeeds: no *Join* button, no `#join-code` input.
- Member role with an answer of no addresses: *Join* stays disabled with a name and a code filled in.
- A pending copy disables *Make a new code*; a saved address change removes the shown code.

## Gate

`pnpm -C builder/ui test`, `pnpm -C builder/ui run typecheck`, `pnpm -C builder/ui build`, `cargo fmt --check`,
`cargo clippy -p voice-api --all-targets -- -D warnings`, and `cargo test -p voice-api --lib -- pairing` if your
sandbox can open sockets (say if it can't).
