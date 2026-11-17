# PB8.9d1 fix-up 3: the Verifier's findings, and words that fit any computer

The Verifier (Fable, PASS WITH FIXES) on `a0698d0`, and one red UI test. Change only what is listed; red test
first for each behaviour change. Don't run cargo or pnpm; the reviewer runs every check. Commit nothing; list
the files you changed.

1. **Never lose the recovery-key notice (must-fix).** Radio writes the fleet's recovery key once, at
   `fleet_init`, and no later op names the file again; today a refusal after `fleet_init` drops the path for
   good. In `pairing::invite_with`:
   - Move `delivery_folder_set` **before** `fleet_init` (Radio's contract: it works with or without a fleet
     and needs only the app registered and installed, which `register` and `ready` already ensured). The
     order becomes `hello`, `link_address_list`, `register`, `delivery_folder_set`, `fleet_init`,
     `code_create`. Update the order test.
   - When `fleet_init` made a fleet (answered a recovery file) and a later step fails, the failure's
     sentence ends with ` ` plus `text::recovery(<file>)`. Test: `fleet_init` answers a recovery file,
     `code_create` is refused, and the failure message contains the recovery sentence with that path.
2. **`voice radio name` speaks plainly.** In `pairing::name`, after `ready`, call `radio.register(vec![])`
   (the same comment as in `register`) so Radio never answers `the app is not registered`, then `resign`.
   Map the refusal phrase `not in a fleet` (through the phrase-returning path, as join does) to a new
   `text::NOT_LINKED`: `This computer isn't linked to your other computers yet. Join them first, or make a
   join code here.` Split a `name_in(home, radio, name)` for tests. Tests: a positive one asserting the
   requests are exactly `hello`, `register` (`members: []`), then `resign` with `app: voice` and section
   `{"computer_name":"Work laptop"}`; and `not in a fleet` giving `NOT_LINKED`.
3. **`.ai/DECISIONS.md`:** in the PB8.9d1 entry, replace "Voice writes that section only after Radio
   accepts a join" with "Voice writes that section only once this computer is in a fleet (after a join, or
   with `voice radio name`)". Also add a bullet: the delivery folder is set before the fleet is made, so a
   refusal after the fleet is made can still name the recovery key's file.
4. **Words that fit any computer.** `builder/ui/src/wording.test.ts` refuses the word `Mac` in the UI's
   sources, and Voice's sentences follow the same rule. Replace the example name `Office Mac` with
   `Work laptop` everywhere this slice added it (the `NOT_A_NAME` sentence, every Rust and TypeScript test
   and fixture, `builder/ui/src/test-support.ts`), but not in `plans/briefs/` (those are records).
   `NOT_A_NAME` becomes: `Give this computer a name of 1 to 64 characters, such as Work laptop, without
   symbols like @, #, $ or brackets.`
