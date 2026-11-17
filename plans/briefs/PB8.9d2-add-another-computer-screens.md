# PB8.9d2 brief: *Your other computers*, the screens for adding another computer

Lane V, runner V1, 2026-11-17. Card `1OsrgDza` (split from PB8.9d). Codex implements; Claude reviews.

Read first: `AGENTS.md` (no real person in any file, fixture or string: invented names only, such as Wren, Ari,
Work laptop and Studio desktop), `.ai/PROJECT.md`, `.ai/DECISIONS.md`, `.ai/SECURITY.md`, then
`plans/briefs/PB8.9d1-add-another-computer-engine.md` (the engine this slice puts on screen). Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/frontend/SKILL.md`, `~/.codex/skills/design-guide/SKILL.md`,
`~/.codex/skills/tabler-ui/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md` (every sentence the person sees),
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md` (the join code).

## Why

PB8.9d1 built `voice radio address|invite|join|name` and five app commands, with backend methods already in
`builder/ui/src/backend.ts` (`radioLinkAddress`, `radioSetLinkAddress`, `radioInvite`, `radioJoin`, `radioName`) and
their fakes in `test-support.ts`. No screen uses them. This slice adds one card to the Save step: on the computer
that builds the assistant it makes a one-time join code; on a member computer (one that opened her from GitHub) it
pastes that code and joins. The plan is `2026-11-12-pb8-9-one-assistant-many-computers.md` step 2.2.

One small engine addition: the member form's name field is prefilled with this computer's own name, and nothing
gives the UI that name today. `voice_save::computer::name()` (`builder/save/src/computer.rs`) already reads it.

## What to build

### 1. Rust: `LinkAddress` carries this computer's name, for the app only

- `builder/api/src/dto.rs` `LinkAddress`: add `pub computer_name: Option<String>` after `addresses`, with the doc
  comment `/// This computer's own name, the join form's first guess. Only the app's commands fill it.`
- `builder/api/src/pairing.rs` `addresses()`: set `computer_name: None`. The app wrappers at the bottom of the file
  fill it: `radio_link_address()` and `radio_set_link_address()` map their `Ok` answer through one private helper
  `fn named(address: dto::LinkAddress) -> dto::LinkAddress` that sets `computer_name: voice_save::computer::name()`.
  `link_address`, `set_link_address`, `set_address_in`, `register`, the CLI and every existing test stay as they are
  (the CLI never prints the name; tests keep `None`, so no test depends on the host's name).
- Regenerate `builder/ui/src/types/generated/LinkAddress.ts` by running `cargo test -p voice-api --lib` (ts-rs writes
  it); never hand-edit it. Commit the regenerated file.
- In `test-support.ts`, both radio address fakes answer `{ port: 7341, addresses: ['10.0.0.5'], computerName:
  'Studio desktop' }`.

No other Rust change, no new app command, no new dependency.

### 2. New `builder/ui/src/screens/OtherComputers.vue`

Props: `assistant: Assistant`. Match `TurnOn.vue` and `OpenSaved.vue`: Vue 3 `<script setup lang="ts">`, Tabler
classes as they use them (`card mt-4`, `card-body p-5`, `alert alert-* ` with `role`, `btn-list`, `form-label`,
`form-control`, `form-hint`, `text-secondary`), every backend failure through `failureMessage(failure, <fixed
fallback>)`, one `action` ref (`'' | 'address' | 'saveAddress' | 'invite' | 'join'`) and a `busy` computed; every
button and field of the card is disabled while `busy`, and each action returns early when busy. No new CSS or
colours; `user-select-all`, `text-break` and `d-block` are Bootstrap utilities Tabler already ships.

**On mount** call only `backend.saveOptions({ slug })`, to learn the role: `memberComputer === null` is the computer
that builds her, anything else a member computer. While loading: `<div class="spinner-border" role="status"><span
class="visually-hidden">Checking {{ name }}…</span></div>`. Failure: `alert alert-warning` with the message
(fallback `` `Voice couldn't check ${name}. Try again.` ``) and a **Try again** `btn btn-primary` that loads again.
**Radio is never asked anything until the person clicks**, so opening the Save step never starts Radio.

Heading `<h2 class="mb-3">Your other computers</h2>` for both roles.

**The computer that builds her:**
- `<p>Link another computer to this one, so {{ name }} can learn from what you do there too. Both computers need
  to be on your VPN, and the other one needs {{ name }} first: there, choose Make a new assistant, then Open one you
  saved on GitHub.</p>`
- A `btn btn-outline-primary` **Add another computer** (label `Checking…` while it runs) calls
  `backend.radioLinkAddress()`. Fallback: `Voice couldn't read this computer's VPN address. Try again.` After a
  success the button is gone and the address block (below) shows.
- Once an address is set, a `btn btn-primary` **Make a join code** (label `Making it…`; after the first code
  **Make a new code**) calls `backend.radioInvite({ slug })`. Fallback: `Voice couldn't make a join code. Try
  again.` A failure's message shows whole (`alert alert-warning`, `role="alert"`), never cut: the engine appends
  the recovery-key sentence to it when Radio made a key before the failure.
- A code shows as:
  - `<label class="form-label" for="join-code-shown">Join code</label>` and
    `<code id="join-code-shown" class="d-block user-select-all text-break">{{ code }}</code>`;
  - a `btn btn-outline-secondary` **Copy** that calls `navigator.clipboard.writeText(code)`; on success the label is
    `Copied` until the next code; on failure (it throws, or `navigator.clipboard` is missing) the warning
    `Voice couldn't copy the code. Select it and copy it yourself.`;
  - the expiry sentence: `It works once, until <time>.`, where `<time>` is
    `new Date(expiresAt * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })`, or
    `It works once, for a short while.` when `expiresAt` is not a finite number above 0 (the CLI's two sentences);
  - `<p>On the other computer, open {{ name }}'s Save step, paste this code under Your other computers, and choose
    Join.</p>`;
  - **when `recoveryFile` is not null, every time:** `<div class="alert alert-warning" role="alert">` with exactly
    `` `Radio saved a recovery key for your computers in ${recoveryFile}. Move it into your password manager, then
    delete that file.` `` (the engine's `text::recovery`, `builder/api/src/radio.rs`, word for word), from a new
    `builder/ui/src/radio-words.ts` that exports `recoverySentence(path: string): string`, so the test can name it.
- A new code replaces the old one on screen (and resets `Copied`).

**A member computer:**
- `<p>Link this computer to the one that builds {{ name }}, so {{ name }} can learn from what you do here too. Both
  computers need to be on your VPN.</p>`
- A `btn btn-outline-primary` **Link this computer** (label `Checking…`) calls `backend.radioLinkAddress()` (same
  fallback as above). After a success, the address block, then the form:
  - `<label class="form-label" for="computer-name">A name for this computer</label>`, an `<input id="computer-name"
    class="form-control" maxlength="64">` prefilled with `computerName ?? ''` from the answer (only the first time;
    never overwrite what the person typed), and `<div class="form-hint">Your other computers show it by this
    name.</div>`;
  - `<label class="form-label" for="join-code">Join code</label>`, an `<input id="join-code" type="text"
    class="form-control" autocomplete="off" autocapitalize="off" spellcheck="false">` and
    `<div class="form-hint">Make one on the computer that builds {{ name }}, on its Save step, then paste it
    here.</div>`;
  - a `btn btn-primary` **Join** (label `Joining…`), disabled while busy, while there is no address, or while the
    trimmed name or the trimmed code is empty. It calls `backend.radioJoin({ slug, code: code.trim(), name:
    name.trim() })`. Fallback: `Voice couldn't join the other computer. Try again.`
  - Success: clear the code field, hide the form, and show `<div class="alert alert-success" role="status">This
    computer joined as {{ joined.computerName }}.</div>` (the CLI's sentence).
  - Failure: the message whole in `alert alert-warning` (`role="alert"`); both fields keep what the person typed.

**The address block (both roles):**
- With addresses: `<p>This computer's VPN address: {{ addresses.join(', ') }} (port {{ port }}).</p>` (the CLI's
  sentence) and a `btn btn-link p-0` **Change it** that shows the field below.
- With none, or after *Change it*: `<p>This computer has no VPN address yet.</p>` (only when none), then
  `<label class="form-label" for="vpn-address">This computer's VPN address</label>`, `<input id="vpn-address"
  class="form-control" inputmode="decimal" autocomplete="off" spellcheck="false">`,
  `<div class="form-hint">The address your VPN gives this computer, such as 10.0.0.5.</div>` and a
  `btn btn-primary` **Save address** (label `Saving…`, disabled while busy or the trimmed field is empty) that calls
  `backend.radioSetLinkAddress({ address: field.trim() })`. Fallback: `Voice couldn't save that address. Try
  again.` Success replaces the shown address, port and (if the person hasn't typed one) the name guess, and hides
  the field; failure shows the message and keeps the field.

**Clearing messages:** each action clears the card's `error` (and `copied`) at its start, as `TurnOn.vue` does.

**Never** put the join code in a log, `console.*`, a URL, an `aria-label`, a `title`, `localStorage` or any state
outside this component; it lives only in the component's refs and goes away when the card does.

### 3. `builder/ui/src/VoiceApp.vue`

In the Save view, right after `<Save …>`: `<OtherComputers :key="`computers-${assistant.slug}-${resets}`"
:assistant="assistant" />`.

## Tests (red first: each must fail on today's code for the right reason)

Vitest with `@vue/test-utils`, using `test-support.ts`'s `fakeBackend`, `screen`, `button`, `deferred` and
`assistant` exactly as `OpenSaved.test.ts` and `TurnOn.test.ts` do. Invented data only. New
`screens/OtherComputers.test.ts`:

- mount calls `saveOptions` with `{ slug: 'example' }` and nothing on Radio (`radioLinkAddress`, `radioInvite`,
  `radioJoin`, `radioSetLinkAddress` not called); a load failure shows its message and *Try again* loads again; an
  `Error` shows the fallback, never its text (`private detail`);
- builder (`memberComputer: null`): *Add another computer* calls `radioLinkAddress` once, shows the address sentence;
  *Make a join code* calls `radioInvite({ slug: 'example' })`, shows `Making it…` and disables every button while
  pending (`deferred`), then the code, the `until` sentence for a fixed `expiresAt` (compute the expected time with
  the same `toLocaleTimeString` call), *Make a new code*; `expiresAt: 0` gives the short-while sentence;
- `recoveryFile: '~/example/radio-recovery.key'` shows `recoverySentence` of it in a `role="alert"`; `null` shows no
  recovery sentence; an invite failure whose message carries the recovery sentence shows the whole message;
- *Copy* calls `navigator.clipboard.writeText` with the code (stub it with `vi.stubGlobal` or
  `Object.defineProperty`, restored after) and shows `Copied`; a rejecting `writeText` shows the copy warning;
- no addresses: the no-address sentence and the field; *Save address* is disabled when empty, calls
  `radioSetLinkAddress({ address: '10.0.0.7' })` for `'  10.0.0.7 '`, then shows the new address; a failure keeps
  the field and shows the message; *Change it* shows the field when an address exists;
- member (`memberComputer: 'Studio desktop saves Example; this computer takes each new version.'`): *Link this
  computer* shows the name field prefilled `Studio desktop`, a typed name is not overwritten by a later address
  save; *Join* is disabled until name and code are both non-blank; it calls `radioJoin({ slug: 'example', code:
  'rj1.example', name: 'Work laptop' })` for padded inputs, shows `Joining…`, then `This computer joined as Work
  laptop.` and the code field is gone; a refusal (`{ message: 'The other computer turned that code down. …' }`) shows
  that sentence whole and both fields keep their text;
- the member card never shows *Make a join code*, and the builder card never shows *Join*;
- the join code never appears in an attribute of any element (`wrapper.html()` holds it only as the shown `<code>`
  text on the builder, and only as the input's value on the member).

`VoiceApp.test.ts`: the Save view shows *Your other computers*.

`builder/ui/src/app/tauri-backend.test.ts` and any fake that builds a `LinkAddress` compile with the new field.

Rust: run `cargo test -p voice-api --lib -- pairing radio` (if your sandbox can't run it, say so; Claude will).

Mutation-check each claim (`cp` backup, `cmp`; never `git checkout --`): at least the role switch, no Radio call on
mount, the recovery sentence, the expiry fallback, the Join disabled rule, the prefill-only-once rule, the code
cleared after a join, and the busy guard.

## Never

- No real person's name, handle or text anywhere; no `Mac` or `Finder` in a non-test source (`wording.test.ts`).
- No new dependency, no new CSS, no inline colours, no new app command; never hand-edit `types/generated/`.
- Never show a raw error's text, a token, or a path the engine didn't put in its sentence (the recovery file is the
  engine's own path and is meant to be shown).
- Never log, persist or put the join code in an attribute.

## Gate

`pnpm -C builder/ui test`, `pnpm -C builder/ui run typecheck`, `pnpm -C builder/ui build`, `cargo fmt --check`,
`cargo clippy -p voice-api --all-targets -- -D warnings`. Don't commit; Claude reviews and commits.
