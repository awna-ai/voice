# PB8.9d1 fix-up 2: the security review's three lows

The security review of PB8.9d1 (Opus, PASS WITH FIXES, no high or medium). Change only what is listed.
Don't run cargo or pnpm; the reviewer runs every check. Commit nothing; list the files you changed.

1. **`.ai/SECURITY.md`, the row *Voice installing Radio and talking to it*:** replace the sentence
   `Join writes nothing before Radio accepts the code.` with
   `Voice writes none of its own files before Radio accepts a join code; it may first install or start Radio and register with it, which writes Radio's own state.`
2. **The same row, the copies that aren't zeroed:** where it says `radio_client`'s own `Request` and
   `Response` print the code in `Debug` and their copies aren't zeroed, also name the copies PB8.9d1 adds:
   `dto::Invite`'s code (copied out of `JoinCode` for the screen and `voice radio invite`), and
   `dto::RadioJoinRequest`'s code with the app's request payload in the window and over the app's IPC.
   One clause; keep the row's style.
3. **`NOT_A_NAME` says what the check takes.** `valid_computer_name` (`builder/save/src/computer.rs`)
   allows 1 to 64 characters with no line breaks, no hidden characters and none of
   `@ < > { } [ ] ( ) # \ ` " $`. Change `radio::text::NOT_A_NAME` to:
   `Give this computer a name of 1 to 64 characters, such as Office Mac, without symbols like @, #, $ or brackets.`
   Update any test that spells the old sentence out (tests that compare against the constant need nothing).
   Add one line to the same SECURITY.md row: the name is the person's own text and travels to every
   computer in the fleet, so the slice that reads `computer_name` back from other computers' records
   treats it as untrusted and checks it again.
