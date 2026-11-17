# PB8.9d1 fix-up 1: the review's findings

Claude's review of the PB8.9d1 change (brief `PB8.9d1-add-another-computer-engine.md`). Change only what
is listed. Red test first for each, in `builder/api/src/pairing/tests.rs`.

1. **Setting the address starts Radio when it isn't running.** It is the first thing a person does on a
   member computer, so `pairing::set_link_address(home, address)` runs `ready(home, &radio)` (the same
   install-once-then-hello step invite and join use) after the local parse and before
   `link_address_list`. `set_address_with` keeps taking only a `Radio` so its tests stay as they are; add
   one test that a `set_link_address`-level call on a fake Radio sends `hello` before `link_address_list`
   (split a `set_address_in(home, radio, address)` if that is what makes it testable, as invite and join
   are).
2. **Never remove the old address for one Radio would refuse.** Setting removes the old same-family
   address and then adds the new one, so an address Radio refuses would leave the computer with none.
   Before any request, refuse with `text::NOT_AN_ADDRESS` what Radio's `link_address_add` refuses
   (CONTRACT.md at v0.3.0): an unspecified address (`0.0.0.0`, `::`), a multicast one, the IPv4
   broadcast address `255.255.255.255`, and an IPv6 link-local one (`fe80::/10`). Check after mapping an
   IPv4-mapped IPv6 address to IPv4, as the code already does. Test each of the five with no request sent.
3. **The missing-home sentence.** The five `radio_*` entry points pass `radio::text::NOT_RUNNING` to
   `assistants::home`, so a missing `HOME` would say Radio isn't running. Add
   `text::NO_HOME = "Voice can't find your home folder, so it can't reach Radio."` and pass that.

Rules as in the first brief (skills, public repo, no new dependency, never log a join code). **Don't run
cargo or pnpm**: the machine is under a load hold and the reviewer runs every check. Commit nothing; list
the files you changed.
