# Decisions: voice

## 2026-11-17: sending history to the build computer (PB8.9e1)

1. The build computer is the fleet's one accepted founder (no `invited_by`); send by its id.
   No founder or multiple founders refuses the send.
2. Only this computer's already filtered `decisions-claude-code.jsonl` travels.
3. One private `pending-send.jsonl` replaces any older pending copy before sending. Keep it on
   failure; remove it only after Radio answers with the sent size.
4. Names are `history-<Unix nanoseconds>-<process id>.jsonl`, decimal, with no random source.
   Taking accepts only `^history-[0-9]{1,20}-[0-9]{1,10}\.jsonl$`; a name collision means busy.
5. Use the authenticated delivery key's first 16 hex characters in `decisions-from-<peer>.jsonl`.
   Keep the computer's supplied name for status only, never a path.
6. Preserve every existing record; add new string ids once, re-run the member's filters through
   `privacy::privacy` (as `corpus::merge` does), and sort by timestamp and id. A shorter read
   never shrinks the computer's history.
7. Page Radio deliveries after the saved cursor, 500 at a time until empty. Open only real files
   without following links, at most 64 MiB. Merge, delete, then save the cursor and computer's
   count. Refused files are deleted and passed; missing files are skipped. Repeating is harmless.
8. Status lives privately in `last-sent.json` (`at`, `to`, `bytes`) and `deliveries-taken.json` (`after`,
   `computers` keyed by authenticated key, with `name`, `at`, `records`). These are 0600.
   Export byte and record counts as TypeScript numbers, matching the JSON bridge.
9. Forget already removes the entire state folder recursively, including pending copies, both
   status files, received histories and the delivery folder; no new deletion list is needed.
10. Raw history stays in the assistant's state folder and travels only from a member computer to
    the build computer over Radio's link (plan D-1), landing in that computer's state folder.
    Nothing else of it leaves the computer.

Send and take use the existing assistant state lock to prevent overlapping writes. A local status
read makes no Radio call. The CLI shows pending sends, the last destination and time, and received
computer counts; the exact status sentences were not prescribed by the brief.

## 2026-11-17: adding another computer (PB8.9d1)

- Setting this computer's VPN address replaces the existing address of the same family;
  setting the same address again leaves it in place.
- One assistant per computer takes deliveries: Radio keeps one delivery folder per app,
  so Voice refuses an invite while another listed assistant has a `deliveries` entry.
- Voice initializes the fleet as `voice`; an already linked computer keeps its fleet and
  produces no new recovery file.
- The delivery folder is set before the fleet is made, so a refusal after the fleet is
  made can still name the recovery key's file.
- A computer's name travels in Voice's section of Radio's signed computer record, as the
  single JSON key `computer_name`. Voice writes that section only once this computer is in a fleet
  (after a join, or with `voice radio name`);
  if naming fails, `voice radio name` lets the person try again.

## 2026-11-12: look for an older assistant before making one (PB8.7a)

Making an assistant now looks for an older one with the same slug and refuses to reuse it:

- A private state folder counts when it holds files recursively, without following links;
  a link (including a dangling one) or something other than a folder counts too, but an empty
  folder does not. Top-level `decisions.jsonl` and `live.jsonl` are reported by name only.
- An existing skill counts unless Voice's readable install record claims that exact skill
  folder for the same slug; a record that cannot be read does not claim it.
- Lines in `CLAUDE.md` naming `/<slug>` as a word count outside all Voice blocks, without
  regard to ASCII letter case; the read is capped at 4 MiB and retains line numbers.
- An unclaimed, real skill folder's regular `SKILL.md` (at most 1 MiB) can identify the old
  folder through its first `--member` path: absolute or home-relative, ending in `/member.json`,
  with no links followed and a readable member whose slug matches.

The check writes nothing. PB8.7b brings an older assistant in after a backup; this slice only
finds and reports it. This replaces VO2's "Nothing is migrated" only for this case.

## 2026-11-09: Claude Code keeps its own login file in Voice's profile (VF3)

The owner chose option 1 of V-Q15, reconfirming the login file chosen by V-Q10 on 2026-10-31.
The sandbox may start `/usr/bin/security`, but the Keychain stays closed: no Keychain services
are allowed and `Library/Keychains` is denied. When the Keychain refuses the write, Claude Code
keeps its own `.credentials.json` (0600) in Voice's profile. Voice never reads, copies or passes
that login, excludes the profile folder from Time Machine, and signs out through Claude Code's
own `auth logout`.

The RAM-volume option was rejected for its memory cost; the Voice-held-login option was rejected
because Voice would hold the login. Console sign-in on macOS waits for VF5: it creates an API key
but cannot save it with the Keychain closed, so Voice refuses it before starting Claude Code.

## 2026-10-23: a public kit with no members

- **The repo ships no member.** Only `crew/_template/` and the build. Builders keep their member in a
  private copy of their own; a fork of a public repo is public, so the README says not to fork.
- **Miners are generic.** A script that mines a person's messages takes the people, their labels,
  the workspace and any filters from a config file outside the repo, and its tests use fictional
  fixtures.
- **No fixed paths.** Scripts locate the repo from their own path; docs never name a clone folder.
- **The folder keeps its name.** `crew/` and `crew_decisions.py` stay as they are; the docs say
  "AI assistant" and "member".

## 2026-10-23: a route for a principal with no Claude history

- **Two sources replace the history:** the principal's own documents, distilled into a lens
  (`documents` plus `lens-brief.md`), and question rounds answered on a form an adult relays
  (`round`). The kit names no principal; the case behind it is anyone without a Claude account of
  their own, such as someone under 18.
- **A round trains unless imported with `--heldout`;** a held-out round keeps that split through
  `merge --rounds` and never moves the history's cutoff, so the existing backtest scores the
  model blind on it. A whole round is held out or none of it.
- **`install.sh --no-history` never mines the Mac it runs on.** That Mac's history is the adult's.
- **PDF text comes from poppler's `pdftotext` when it is installed,** not from a Python
  dependency: the kit stays standard-library only, and a missing tool is a skip that says how to
  fix it.

## 2026-10-25: extra sources, each behind a consent date

- **Sources live in the gitignored `private.json`** as a `sources` list (kind, path, the kind's
  options); `member.json` may not hold one. A source is read only once its `consent` date is set,
  which the build writes after the principal's yes.
- **Mail comes from a local mbox** (Google Takeout), sent mail only, never a live mail connector:
  the kit reads only local files, and the agent never sees received mail. The filters run per
  paragraph, so one address in a mail drops that paragraph, not the mail.
- **Project folders feed the lens only,** read in place; files Claude wrote or edited start as not
  mine. Documents are not decisions, so they stay out of the decision corpus.
- **The live log trains, never tests** (`decisions-live.jsonl` is Train in install.sh and in the
  Rust `discover`), and skips a question already mined from the history.
- The Rust port of `sources` waits for the app's sources screen.

## 2026-10-26: shipped decision commands (PB8.1)

An app-made assistant's SKILL.md runs the `voice` shipped inside the app
(`Contents/MacOS/voice`), named as one quoted path when the assistant is made; the guided
build keeps `scripts/crew_decisions.py`. The template names both through `{{SIMILAR}}` and
`{{RECORD}}`. An assistant made before this keeps its old SKILL.md; make it again to get the new one.

## 2026-10-26: texts go to the decision commands on stdin (PB8.3)

The SKILL.md's look-up and record commands take their texts as one JSON object on stdin
(`--stdin`, a quoted heredoc), never as shell words, in the app's `voice` and the
guided build's Python alike; the flags stay for people typing the commands. JSON over one field
per line because a text can hold line breaks and a line that matches the heredoc's end. An
assistant made before this keeps its old SKILL.md; make it again to get the new one.

## 2026-10-31: files the app never opens (VO8.3)

The app checks every file of the person's it is about to read against a list of password and key
files (`voice_core::deny`), as given and with links resolved, without regard to letter case,
and never opens one that matches. The list is built in (the VO plan's list) plus the person's own
`filters.denied_paths` in member.json or private.json: names, or `~/`/`/` paths, in plain
letters only, because the app has no Unicode normalisation and macOS writes accented names
decomposed. It lives in core because every reader depends on core. Only the app applies it: the
Python kit's own readers don't, and its `private.json` reader refuses a `denied_paths` list, so a
member that sets one is read by the app only. A hard link to a key file under an ordinary name
gets past a path list; the text filters still apply to what is read.

## 2026-10-31: one provider interface for every model call (VO8.1)

Every place the app asks a model takes a `Provider` (`voice_agent::provider`), not
Claude Code itself: an id, a name, where its data goes (`Reach`: your own machines, Anthropic, a
third party) and one tool-less call. A `Brief` has no tools field, so no provider can be handed
one. The person's own Claude Code is the first provider, unchanged. Finding, checking and signing
in to Claude Code stay Claude Code's own, because a later provider will have none of them.

## 2026-10-31: where each source's data may go (VO8.2)

Every brief names the sources of what it carries, and every model call goes through the privacy
guard (`voice_agent::guard`): it hands the brief only to a provider whose reach every
source's ceiling allows, seals it for that provider, and refuses in plain words when none is
allowed, before anything is spent. A provider takes only a sealed brief and checks the seal again
first. The ceilings (`local-only`, `anthropic`, `any-consented`) default to the VO plan's gate
answer Q6: the person's Claude history, the assistant's own folder and the copy of their skills
may go to Anthropic; mail, calendar, contacts, family and legal skills stay on their own
machines. Two choices made at pickup:

- **What the person types into Voice is a source of its own** (`typed`: interview
  answers, notes on answers, a panel's question and steers), defaulting to `anthropic` because
  it is typed into steps that run on Claude Code. The person can keep it home like any other.
- **Ceilings live in member.json** (`"ceilings": {"<source>": "<ceiling>"}`), changed with
  `voice ceilings show|set`; private.json may not hold them, so one file is the only
  setting. The app gets its screen in a later slice.

`any-consented` reaches no third party yet: there is no third-party provider and no record of
the person agreeing to one. The build names mail when mail mined by the guided build can be in
its corpus (a `decisions-src-*` file in the state folder, or an `mbox` source in the config),
because merged records do not say where they came from. Claude Code's child no longer inherits
the variables that switch it to another provider or endpoint. The interview's draft is labelled
with the assistant's own folder only: it sends the kinds of decision and how many of each the
last build found, numbers but no record.

## 2026-10-31: a model server on the person's own machines (VO9)

The second provider is any model server on this computer or the person's own network that speaks
the OpenAI-style `POST /v1/chat/completions` with a JSON-schema reply (Ollama, llama.cpp's
`llama-server`, LM Studio, vLLM all do), set up per assistant with
`voice self-hosted set --url … --model …` and kept in the assistant's state folder
(`self-hosted.json`, 0600), never in member.json, because the address belongs to this Mac.
Choices made at pickup:

- **Claude Code first, the server second.** A step goes where it goes today unless its data is
  kept home; a person who wants everything local sets every ceiling to `local-only`.
- **This computer only, unless the person says their network.** The check is on the address
  connected to, not the name typed, for every address a name resolves to at each connection:
  loopback by default; with `--network`, also `10/8`, `172.16/12`, `192.168/16`, IPv6 unique-local
  (`fc00::/7`) and link-local.
  A private address isn't proof of the person's own network (on other Wi-Fi a stranger's device
  can answer at the same address or name), so `--network` is the person's choice and the words
  say so. `100.64/10` is left out (carrier-grade NAT is shared with strangers), so a Tailscale
  address doesn't count yet. No proxy, no redirects.
- **No token yet.** The common servers need none on a home network; a token kept in the
  Keychain is a later card.
- **A call costs $0 but still waits for the daily limit,** so one limit governs every step and
  every refusal reads the same.
- **The build cache keeps Claude Code's keys** (a rebuild reuses what the person already paid
  for) and keys another provider's replies apart; the backtest entry names who answered.
- **Panel turns take the narrowest ceilings across every seat,** because a seat's post carries
  its folder's data into the other seats' briefs.

The app's screen for it is a later card, as for ceilings.

## 2026-10-31: the operating-system sandbox is Seatbelt, one profile per child (VO11)

The VO11 spike (`docs/spikes/VO11-os-sandbox.md`) decides how the broker is packaged on macOS:

- **Seatbelt, not App Sandbox.** Each sandboxed process is started through `/usr/bin/sandbox-exec`
  with a profile kept in the repo and its folders passed as parameters. App Sandbox has a single
  on-or-off network switch, gives a child the whole app's grants, can't run a Claude Code
  installed outside the app, and needs a signed app (PB-D3).
- **Siblings, never nested.** macOS refuses a sandbox inside a sandbox, so an unsandboxed
  launcher (the app) starts each sandboxed piece itself: the `claude` child first (VO11.1), the
  broker's reading in a helper of its own later (VO11.2).
- **One port out.** A sandboxed child may connect only to a proxy on this computer, which allows
  `CONNECT` to port 443 of a listed name only. Claude Code follows `HTTPS_PROXY` and has no SOCKS
  support.
- **The profile's own tests are part of the checks.** `builder/agent/tests/sandbox_profile.rs`
  runs on every Mac in `scripts/ci-local.sh` (`VOICE_REQUIRE_SANDBOX=1`), since Apple
  doesn't document the profile language and an update could change it.
- **Not wired in yet.** Nothing runs under the profile until VO11.1, which waits on the owner's
  answer about where Claude Code's login lives inside it (the profile reaches no Keychain).

## 2026-10-31: the anonymizer copies a skill whole or not at all (VO3)

`voice anonymize` copies the skills a person agreed to, with private details taken out
(`voice_ingest::anonymize`, `::skills`; brief `plans/briefs/VO3-anonymizer.md`):

- **A yes per skill, kept in private state** (`~/.claude/state/<slug>/anonymizer/skills.json`),
  with the folder it was given for: a skill that moves waits for a new yes. claude.ai's synced
  skills are never offered; naming one is the yes. The person's `never_copy` list beats a yes.
- **The rules are data.** Built-in detectors (emails, phone and long numbers, secrets, other
  people's home folders, private hosts and addresses, ticket ids) are always on; the person's
  terms, patterns, allow list, never-copy list, dropped files and project folders live in the
  `anonymizer` object of their gitignored `private.json`, never in `member.json`.
- **Transform, then re-scan, then refuse.** Invisible characters and `<!-- private -->` sections
  go, home folders become `~`, a listed term becomes a placeholder (`<company-1>`) that keeps its
  number for good (`anonymizer/placeholders.json`, private). Then everything is checked again,
  terms included (wrapped over a line, or inside a longer word); any hit refuses the whole skill.
  `LOCAL.md`, `LEARNINGS.md`, hidden, password and key files, links and files over 1 MiB are left
  out unread.
- **The report names file, line and kind**, the line being the source file's, never the text.
- **`skills list` shows ids and folders as they are**, numbered (`1. <id>…`): the person needs
  them to agree. `anonymize`'s report never shows a found term; a skill whose name holds one is
  named by its number in that list, and a file path's private parts print as `*`.
- **A term is found inside longer words and through punctuation** (`mypebble-freight.com`), when it
  has 4 or more letters or digits, and `ss` and `ß` match each other. An allow entry exempts a hit
  only when one entry covers all of it; a file or folder that can't be read is left out as
  `unreadable`, and the run goes on.
- **A term needs a letter or a digit** (`\p{L}` or `\p{N}`, the classes the separator leaves out).
  Words with neither inside a term (a sign-off dash)
  are left out of its search, and a term with no letter or digit at all is refused when the rules
  load. Separators between a term's words have no length cap.
- **No hashes yet.** Refreshing on change, and the review screen for new skills, are VO4 (the
  mirror and its hashes: VO4.1, below).

## 2026-10-31: the app is Voice, and nothing is migrated (VO2)

The app in `builder/` was Persona Builder; it is Voice now, everywhere (brief
`plans/briefs/VO2-rename-to-voice.md` holds the whole name map).

- **The names:** the command is `voice`; the crates are `voice-core` (library `voice_core`) and
  `voice-<x>`; the app is `Voice.app` (`voice-app` inside, with the `voice` command beside it);
  the screens are `@voice/screens` with `VoiceApp` at the root; variables are `VOICE_*`; the
  bundle id is `dev.voice.desktop`, still a placeholder until signing.
- **Nothing is migrated.** The app is not released and no assistant exists outside invented test
  ones, so every name on disk moved and nothing reads the old ones: Claude Code's own sign-in
  (`Application Support/Voice/claude-code`), the GitHub sign-in in the password store
  (`voice.github`), the CLAUDE.md markers (`<!-- voice:`), the report schema (`voice/report/1`).
  An older build's files under the old names are left where they are.
- **Turning on uses the shared `~/.claude/voice/`**, which Panels use too: the rules go in
  `assistants/<slug>/RULES.md` and the record is `installed.json`. Turning on creates the shared
  folder (0700) when it is missing and never counts it as its own, so turning off never removes
  it or the panels in it, and an assistant named "Panels" can't land in the panels' folder.
- **The consent wording names Voice, so it is version 2**: a yes given to version 1 is asked again.
- **`.ai/handoffs/persona-builder.md` keeps its name and its old wording:** it is the dated record
  of what was built, under the names of the day.

## 2026-11-01: the skill mirror is fresh, absent, or marked out of date (VO4.1)

- **The mirror lives in private state**, `~/.claude/state/<slug>/mirror/` (0700 folders, 0600
  files), so `voice forget` deletes it with the rest.
- **Hashes of the copies only.** `.manifest.json` lists the SHA-256 of each file as written in the
  copy. A hash of a private source file would let a reader test a guess about what it says.
- **Every refresh starts from nothing**, so no copy made under older rules survives a refresh that
  succeeds. Withdrawing a skill removes the mirror at once.
- **A failed refresh never leaves the mirror looking fresh.** Once the overlap check has passed,
  a failure removes Voice's own copy (only a folder bearing its exact stamp, never a link). Before
  that check (the rules or the saved placeholders can't be read) or when the check refuses, Voice
  removes nothing and only drops the manifest. Each message says which happened, and none tells
  the person to delete a folder that may hold their own files.
- **Voice never empties a folder that overlaps the person's own**: a skill root, anything listed
  in one (even an entry discovery skips), a skill's folder, or a project listed in `private.json`.
- **Readers trust the mirror only with Voice's stamp and a regular, valid manifest**, and refresh
  first anyway (VO4.2).
- **A panel refreshes each seat's mirror before its first round** (VO4.2), in convene and in a
  resume that will run a round, under the state locks the panel already holds. A seat is refreshed
  when it has agreed to a skill, its agreements can't be read, or a mirror is there (or can't be
  checked), so after the start every seat's mirror is fresh, absent, or marked out of date; an
  assistant that never agreed to a skill gets no mirror. A failed refresh never stops the panel:
  one fixed line names the assistant, never a skill, a path or the error.
- **The mirror's path is shown starred** (`~/.claude/state/*/mirror`) whenever the slug holds a
  term, or the rules didn't load and the terms are unknown.

## 2026-11-01: a read-wide turn's fence, and how it is proven (VO1)

- **The fence is Claude Code's own permission rules**, given per turn: `--tools Read,Grep,Glob`,
  no MCP server, `--permission-mode manual` with `--permission-prompts none`, `--add-dir` per
  root, and the built-in deny list as `Read(//**/<name>)` and `Read(//**/<name>/**)` rules in a
  `--settings` file. `docs/spikes/VO1-read-fence.md` has the full argv; `scripts/read_fence_probe.py`
  holds it in one place and a test keeps its rules equal to `voice_core::deny`.
- **Anchored at `//`**, because Claude Code reads `**/.env` as "under the current directory", which
  a folder added with `--add-dir` is not.
- **A turn whose settings weren't applied is refused.** `-p` ignores a broken settings file
  silently, so the caller passes `--debug-file` and requires Claude Code's line saying it added
  exactly the rules sent. The line isn't documented, so this fails closed.
- **Proven on a person's own Mac, attended.** An unattended run's nested `claude` can't sign in, so the
  live runs are the person's: `python3 scripts/read_fence_probe.py run`, which passes only when
  every deny-list canary's read is denied by Claude Code (`permission_denials`) and no canary's
  contents appear in the turn's output. Until both Macs have passed, nothing relies on the fence.

## 2026-11-02: every `claude` runs in the sandbox (VO11.1c)

Supersedes the VO11 entry's *Not wired in yet*. The owner chose a login file in Voice's own
Claude Code folder (V-Q10, option 1), so VO11.1 went ahead in three parts: the proxy (VO11.1a), the
launch as a library (VO11.1b) and the wiring (VO11.1c).

- **One launch per `claude`, each with a proxy of its own.** The check's two runs, each call and
  the sign-in each start `sandbox::Launch` with an `egress::Egress` that lives exactly as long as
  the child. The environment is built from nothing, so no credential, provider switch or endpoint
  of the person's reaches Claude Code. `ANTHROPIC_API_KEY` can't either, so the note saying it might
  bill that key is gone.
- **Each launch gets a fresh run folder, never the session's work folder.** Measured on Mimi:
  a sandboxed `claude -p` leaves a `claude-<uid>/` folder in `CLAUDE_CODE_TMPDIR`, so a reused work
  folder would refuse the next call. A fresh 0700 folder in `runs/` beside the profile is just as
  empty and Voice-owned, and is removed when the launch ends. The work folder is gone.
- **The app opens the sign-in page itself.** Claude Code hands its link to a helper in the run
  folder. Voice checks it (`sandbox::valid_link`), opens it once with `/usr/bin/open`, and never
  shows, logs or returns it. A link that fails the check, or a page that won't open, stops the
  sign-in. `voice claude sign-in` works the same way, and Ctrl-C stops the sandboxed child too.
- **Tests run stub `claude`s directly, never production.** A `#!` script can't run in the sandbox.
  So the `voice-agent` feature `test-stubs`, enabled only from `[dev-dependencies]`, runs a script
  directly with the same arguments and environment. Native programs go through `sandbox-exec` in
  every build. Test builds of the CLI (`cargo test`, or any `--all-targets` build) carry the feature;
  its shipping build and the app's don't. `scripts/ci-local.sh` checks, for every target, that the
  CLI and app builds lack it, and builds them clean without it.
- **Still owed: the walk.** Nobody has yet signed in and asked inside the sandbox on a person's own
  Mac (lane V's walk list). Until that passes, the result rests on the stub tests and the signed-out
  runs measured here.

## 2026-11-02: the app reads through the reader (VO11.2c)

- **The app reads history only through the sandboxed reader**, with exactly the step's source and its permission checked both before launch and inside the reader.
- **Protocol violations are one fixed error.** Output is bounded and progress relayed line by line; a final line must agree with the exit status.
- **Reader deadlines bound silence.** A reader silent for 10 minutes, or keeping output open 5 seconds after its answer, is stopped.
- **Only Voice's fixed sentences reach the screen.** Any other reader refusal becomes the step's own sentence.
- **The export kind is stored and enforced on the opened handle.** A choice without its kind must be made again.
- **Missing projects need no reader.** With no `~/.claude/projects`, the app returns the empty answers itself (after the lock for the read) and reads nothing.
- **macOS only.** Other platforms keep reading in-process; skills wait for VO11.2d.

## 2026-11-02: the reader starts with only stdin, stdout and stderr (VO11.2c.2)

- **The reader starts through `posix_spawn` with close-by-default**, with only fds 0 to 2, instead of `Command`: Rust's macOS spawn can hand it another child's descriptors.
- **History and an export folder are read through the listed folder's descriptor** (`openat` with `O_NOFOLLOW`), not by path: a folder swapped for a link between listing and opening can't redirect the read (VO11.2c.1).
- **The reverse case is a retry.** Another child started while the reader's pipes are being made (marked close-on-exec a step later; macOS has no `pipe2`) can still inherit either end of its stdin or stdout; the deadlines (5 seconds after an answer, 10 silent minutes) bound the wait and the person tries again.

## Recordings

- Recordings of a real person's voice never enter git. `.gitignore` blocks audio; never weaken it.

## 2026-11-02: skills through the reader (VO11.2d)

- **Link targets are sources one by one and must hold a skill.** Their parent folders would let the reader see unlinked neighbours.
- **A refused source drops its skills.** It does not fail the skills screen. No source may be inside any assistant's state or crew folder.
- **A panel asks with `refresh-held`.** It already holds each seat's state lock, so the lock does not pass across processes.

## 2026-11-02: permission for each capability, and a log the person can read (VO14.1)

A grant says whether Voice may read a source for one assistant; its ceiling still says where that
source's data may go. Every grant is dated on the person's local day, scoped to a window, and
revocable. It lives in `grants.json` in the assistant's private state folder, never member.json:
permission belongs to this person and this computer, so copying an assistant doesn't copy consent.

- **Reads only.** The first capability is `calendar.read`. A write or send will need the person's
  confirmation for each action (A4), never a standing grant.
- **Every grant, revoke, read and refusal leaves an audit line.** `audit.jsonl` holds only the UTC
  time, event and capability, plus the window for a grant, the step and item count for a read, or
  the step and fixed reason for a refusal. Never a path, title, name, error text or source content.
  The person reads it with `voice audit show`; `voice grants show|give|revoke` manages permission.
- **Damaged permissions refuse reads.** A grant or revoke that can't be logged restores the exact
  previous permissions file (or removes the new file when there wasn't one). Permission nobody
  can see in the log must not stand.
- **Calendar first, from an exported file (VO14.2),** following mail's local-export precedent,
  rather than a live connector. Nothing reads a calendar in this slice. The app's screens come
  in VO14.3.

## 2026-11-02: the calendar, from a file the person exported (VO14.2)

- **The person chooses an export:** one `.ics` file or a folder of them, kept as
  `calendar-export.json` in the assistant's private state. Like mail, it needs no live connector
  or macOS calendar prompt. `voice calendar choose|show` makes the choice and shows the read;
  the app's screens wait for VO14.3. Choosing needs no agreement to read conversation history:
  `calendar.read` is this source's permission.
- **Only the sandboxed reader reads the export**, through `read-calendar`, with that export as
  its only source. Off macOS, choosing still works but reading refuses. The broker checks the
  grant before launch; the reader loads it again and takes the window from that grant, never
  from the request. It refuses a requested day more than one day from its own UTC date. The
  broker checks the returned window, event count, dates, overlap and cleaned text before use.
- **No read without a trace.** After checking the answer, the broker logs its step and item
  count. If the append fails, the answer is dropped. The log holds no calendar text or path.
  With no grant and no state folder, the refusal is the fixed no-permission sentence; nothing
  is created and there is nowhere to log it. A reader refusal after opening the export, such
  as a too-big or bad file, leaves no audit line by design: no calendar answer came out.
- **Five fields leave the parser:** title, place, start, end and status, with an all-day flag.
  `DESCRIPTION` is never parsed into a field: meeting links and codes often live there.
  Attendees, organizer, URL, contact, comments, attachments, `X-` properties and all other
  properties outside the calendar rules never leave the parser. The member's personal and
  secret filters check both title and place before any display cutoff; left-out occurrences
  are counted. Titles and places are cleaned to one line and cut to 200 characters plus `…`.
- **Times use the system zone database, never a guess.** UTC and named times are converted to
  the person's local time; floating times already mean that local time, and dates stay dates.
  Named zones come from `/usr/share/zoneinfo` (TZif v2 or later, its 64-bit block), never a file's
  `VTIMEZONE`. The broker finds the person's zone from nonempty `TZ`, else `/etc/localtime`,
  outside the sandbox: the reader can't follow that link into `/var/db`. A leading `:` is
  dropped; a zone path uses the part after its last `zoneinfo/`. No loadable zone means refusal,
  with no UTC fallback. Unknown zone names make entries not understood. A spring-forward gap
  moves forward by the gap; a repeated hour uses its first instant. An empty TZif footer, or
  one with daylight-saving rules, gives no offset beyond the last transition; an empty footer
  without transitions gives none anywhere. A fixed-offset footer still works in the future.
- **Recurrence is deliberately bounded.** Daily rules have no `BY` part; weekly rules allow
  plain `BYDAY`; monthly rules allow `BYMONTHDAY` ±1–±31 or ordinal `BYDAY` ±1–±5; yearly rules use
  the start's month and day, with optional `BYMONTH` and `BYMONTHDAY` equal to that month and day.
  Every frequency allows `WKST`, which affects only weekly rules here. `INTERVAL` is 1–1,000; `COUNT`
  is 1–100,000, or `UNTIL` is inclusive. The start counts first, before `EXDATE` removes any;
  missing month days and leap days skip. Expansion uses the start's own clock, then converts.
  Timed `EXDATE` and `RECURRENCE-ID` values match by UTC instant, even across different zones;
  all-day values match by date, and the value types must match the start.
  `RECURRENCE-ID` replaces one occurrence, including a cancelled one; without its series in the
  export it is a single event. An unmatched override whose series is present is not understood.
  Other rules, extra recurrence forms, invalid entries and unavailable future zone information
  are left out and counted as not understood when they could touch the window; old unrelated
  history does not count.
- **The grant's window is overlapping local days**, from today minus its past days through
  today plus its next days, both included. An event that starts earlier but ends within it is
  kept. All-day ends are exclusive in the file and inclusive in the answer; a timed end at
  midnight belongs to the preceding day unless start equals end. Calendar's default ceiling
  stays local-only. This slice sends no calendar to a model and grants no write or send.
- **Caps bound every export:** at most 64 `.ics` files, 64 MiB per file, 128 MiB together and
  200,000 components. An unfolded line over 64 KiB makes its entry not understood, or the file
  bad when outside an entry. Bad UTF-8 or unmatched components refuse the whole read. Files
  must be regular and not hard-linked; links inside a folder are skipped. At most 2,000 events
  come out, earliest first, with `more` when some were cut. Each property has at most 16
  parameters and each entry at most 10,000 `EXDATE` values; exceeding either makes the entry
  not understood. A rule takes at most 500,000 candidate steps, and the whole read shares a
  5,000,000-step budget. Each attempted period (even an empty one) and each candidate uses a
  step. Once spent, remaining entries are not understood when they could touch
  the window. Rules without `COUNT` skip ahead to periods that could reach the window, after
  visiting the start; rules with `COUNT` walk from the start so earlier occurrences still count.

## 2026-11-03: Ask is where a capability's read first reaches a model (VO14.4a)

The person chose it (V-Q12, answer a): one question to one assistant, where they tick which granted
permissions feed it this time. Panel seat briefs stay unchanged, so no other assistant's data is
mixed in and one seat's calendar can't keep a whole panel on local models.

- **What goes in:** the assistant's `decisions.md` (`crew-folder`), the typed question (`typed`),
  today's local date, and for each ticked permission what it read (`calendar.read` adds
  `calendar`). Nothing else: no history, no skills, no other assistant.
- **The guard routes before any source is read or anything is spent.** (The assistant's own
  settings, its `decisions.md` and its model-server settings are read first, to know what to
  route.) A refusal (the calendar ticked and no model
  server set up, by default) reads nothing, logs nothing, opens no session and runs no `claude`.
  The calendar is read only after the route is found, through the sandboxed reader, under the
  grant and its window, with step `ask` in the audit log.
- **Claude Code's sign-in is checked only when the route picks it**, so an ask that goes to the
  person's own model server needs no Claude sign-in. Claude Code must still be installed, as for
  every step.
- **Nothing is kept.** The reply is returned, never written; the only writes are the ledger line every
  model step leaves and the audit log's read line. The reply isn't withheld for looking private,
  because it goes only to the person who asked (a panel withholds posts because they go to other
  assistants); it is cleaned of control and invisible characters (line breaks and tabs kept) and capped at
  4,000 characters.
- **Fixed text only in the system prompt** (it is in the `claude` child's argv): the names, the
  decisions, the question and the events go in the prompt. The decisions, the question and the
  events are fenced as data; the two names go in the unfenced first line, cleaned and cut to 64
  characters, as a panel turn does. At most 500 events go, and
  the person is told when more were left out.
- **One call, no retry.** The app's screen is VO14.4b.

## 2026-11-04: the read-wide fence is proven, on one Mac (VO1)

Supersedes the VO1 entry's *Until both Macs have passed, nothing relies on the fence*.

- **The fence held.** `python3 scripts/read_fence_probe.py run` passed on one Mac with Claude Code
  2.1.292: every deny-list canary inside the granted folder (nested, upper-case and symlinked ones
  included) and the file outside it were refused by Claude Code itself, and no canary's word reached
  a turn run under the `//` rules. The output is in `docs/spikes/VO1-read-fence.md`.
- **One Mac is enough.** The owner accepted that run as the proof, so VO6 and any later read-wide
  turn may rely on the fence now. A second Mac's run is owed for the record and blocks nothing.
- **The `//` form stays required:** on the live run a `**/.env*` rule did not hold for a folder
  added with `--add-dir`, and with no rules `.env` was read.
- **Glob showed names, not contents** (2 of 22 denied names on the run), and **Grep held** there.
  Grep's fence is still "best-effort" in Claude Code's docs, so whether to keep Grep, and whether a
  denied file's name may reach a turn, are VO6's decisions.

## 2026-11-05: the read-wide fence held on a second Mac (VO1.1)

Supersedes the 2026-11-04 entry's *A second Mac's run is owed for the record*.

- **The same result.** `python3 scripts/read_fence_probe.py run` passed on a second Mac with Claude
  Code 2.1.292, from another account's own sign-in and a fresh clone of the public repo. Every
  check line matched the first Mac's; only the Mac's own hooks and plugins (7 and 1, against 18
  and 7) and the cost (US$0.23, against US$0.15) differed. The output is in
  `docs/spikes/VO1-read-fence.md`.
- **Nothing changes for VO6.** The `//` form stays required, Glob again listed 2 of the 22 denied
  names (the probe prints counts, not which ones), and Grep held again; whether to keep Grep, and
  whether a denied name may reach a turn, are still VO6's decisions.

## 2026-11-13: a member computer opens a saved assistant and never saves it (PB8.9b1)

- **One building computer.** The computer that builds an assistant is the only one that saves her to GitHub. A
  computer that opened her from there is a member computer: its `save.json` says `"role": "member"` (a building
  computer's record keeps its old bytes, with no `role`), and Save and every save choice refuse there. An older
  Voice can't read a member record at all, so it can't save from one either.
- **The building computer's name travels in the commit.** Save adds a `Saved-on: <computer name>` trailer; opening
  reads it from the newest commit into `save.json`'s `builder`, so the refusal can name that computer. A copy saved
  before this has no trailer, and the sentence then says "the computer that builds <Name>".
- **Opening takes nothing it doesn't check first.** Only `crew/<slug>/` from `main`, plain ASCII names, no
  dot-files, the deny list, size caps, never `private.json`, and a member whose private folder is
  `~/.claude/state/<slug>`. Everything is staged and checked before an older assistant is backed up or anything
  of the person's is touched.

## 2026-11-14: on a Mac, the Console sign-in is an API key the person pastes (VF5)

Supersedes the 2026-11-09 entry's *Console sign-in on macOS waits for VF5*, and amends PB-D9's "Voice
never reads, sets or passes a credential" and the VO11.1 entry's "`ANTHROPIC_API_KEY` can't either",
for this one key (V-Q15 answer 2, 2026-11-09).

- **The person pastes a Console API key** on Connect Claude or into `voice claude sign-in` (a terminal
  prompt with echo off, or one line on stdin; never an argument). Voice checks its shape (`sk-ant-`,
  never an Admin key's `sk-ant-admin`), keeps it in its own Keychain item (`voice.claude-console`,
  written through `security -i` on stdin, the GitHub sign-in's store), and checks Claude Code reads it
  as signed in before keeping it. Claude Code checks a key with the server only when it is used
  (measured on Claude Code 2.1.311, 2026-11-14: `auth status` reads a made-up key as signed in, and the
  first call fails with "Invalid API key"), so a mistyped key is kept, and the first call says Claude Code
  rejects the key and to sign out, then paste a new one.
- **Voice passes it as `ANTHROPIC_API_KEY`** to the sandboxed Claude Code on each check and call, never
  to the sign-in or sign-out runs. The sandbox doesn't change: the Keychain stays closed to Claude Code.
- **Sign-out forgets the key first**, even when Claude Code can't be found or the stored value is
  unreadable, then runs Claude Code's own `auth logout` (also when forgetting failed, which it then reports), and says it worked only when Claude Code is signed out
  or isn't installed and has left no login file. Each run reads the key from the Keychain as
  it starts, so a sign-out also stops builds already going from using it. A key and a
  subscription login never mix: a refused subscription login is signed out before a key is kept, and
  a key is offered only when Voice isn't signed in. A leftover login file is signed out the same way. A stored
  value Voice can't read doesn't lock the person out: Connect Claude and `voice claude sign-in` still take a new key, and a pasted key of the wrong shape never costs that value. Only Claude Code's reply for a key passed in the environment (`Invalid API key · Fix external API key`) asks for a new key; its other `Invalid API key` reply means its own login, so it reads as signed out.
- **Claude Code keeps nothing of the key:** after a check and a call with a made-up key in the
  environment, no part of it was in the throwaway `CLAUDE_CONFIG_DIR` (measured on 2.1.311).
- **Off a Mac nothing changes:** Claude Code's own `auth login --console` keeps its login itself.
- **Tests never touch the Keychain:** with the agent's `test-stubs` feature the key lives in a 0600
  file in the test's home; `scripts/ci-local.sh` already fails a shipped build that has `test-stubs`.

## 2026-11-15: the permission text says how a subscription stops, so it is version 3 (VF7d)

- **One wording for every build:** Voice stops for the day at the person's daily limit or, signed
  in with a Claude subscription, when Claude Code says the plan is nearly used up (VF7b's stop).
- **The wording changed, so it is version 3:** a yes given to version 1 or 2 is asked again, once.

## 2026-11-19: the repo moved to `awna-ai/voice`

- **The kit's home is `https://github.com/awna-ai/voice`**, transferred from a personal account.
  GitHub redirects the old web and git URLs, so an existing clone keeps working; point it at the
  new URL with `git remote set-url origin git@github.com:awna-ai/voice.git` (or `upstream`).
- **Nothing else moved by hand:** the repo had no Actions secrets or variables, environments,
  deploy keys, webhooks, Pages site, branch protection or rulesets to carry over.

## 2026-11-19: a fenced folder that is a link refuses the whole compartment map (VF31)

- **Refuse, never follow:** when a path under `fenced` is itself a symbolic link, the member does not load
  and every command exits 2, naming the link and the fix (fence the folder it points to, or remove the link).
  The owner picked this over reading the link's target and fencing both spellings: refusing needs only one
  `lstat` of the fence path itself, never anything inside it or the target, and it fails closed. A fence path
  that can't be checked (any error but "not there") refuses the map too.
- **A link in the fence's parent is fine:** `fenced_forms` already fences both spellings of that.
- **Nothing that would need a look inside a fence:** a fence path with a `..` part, or a fence inside
  another fence, refuses the map before any `lstat`, since checking either could read through a link or
  inside a fence. Write fence paths plainly and list only the outer folder.
- **An OS read-deny on a fence stops the tool too:** where a sandbox denies reading a fenced folder, its
  `lstat` fails and every command exits 2. That is the fail-closed choice; a setup that needs both would
  have to allow reading the fence's own metadata.
- **One TODO item per link:** a member config may name a `todo_command` (an argv list, run without a shell).
  A refusal adds one item per linked fence, and none while an open item for that link exists; a done item
  doesn't count. The tool's own output is never shown, since it is the person's private list.

## 2026-11-20: a member's `follow` list names the folders whose links Voice may follow (VF31)

- **The rule is the member's own config:** the compartment map's optional `follow` lists folders (a
  shared projects folder, a wiki). On a fenced folder's path, a link is followed only when it sits at or
  below a follow folder; any other link stops the load (exit 2) before Voice reads it, and the message
  names the link and the two fixes (add its folder to `follow`, or write the fence by its real path).
  With no `follow`, no such link is followed. Voice never builds in a folder layout, and a member never
  has to regroup their folders.
- **Inside the home folder or outside it, the same rule (the owner's pick, V-Q31 A):** the only links
  followed without a `follow` entry are those on the home folder's own path, meaning every folder Voice
  passes on the way to the home folder: the folders above it (such as `/var` on a Mac, when the home
  folder sits under it), the home folder itself, and, when the home folder is a link, the folders on
  the way to where it points. They are matched exactly (no case folding). A fence under another system
  link (`/tmp` on a Mac, say) needs that link in `follow` (`"follow": ["/tmp"]`). So a fence written by
  another spelling of home, or through a link outside home that leads into it, meets the same rule,
  with no check of which folder is which on disk.
- **Follow folders are compared by spelling, never probed:** part by part and exactly (no case
  folding), written by their real path or under `~/`. A follow folder inside a fence, by either
  spelling of the fence, is refused, since a fence's links are never followed.
- **No list that looks inside a fence loads (V-Q31 A):** while it checks, Voice keeps a list of every
  folder it looked at. Once every fence's real place is known, a look at or inside any fence, other than
  that fence's own single check, refuses the map (exit 2), with a message naming the fence. The
  comparison is the one the walk's own guards use, so a look by a Mac's `/System/Volumes/Data/...`
  spelling inside a plainly written fence counts as inside it. It matches one way only: a fence written
  in the data-volume spelling is not matched by a plain look (VF35). And because the guards' match also
  takes the folder name as text, a fence whose name begins another folder's name (`notes` beside
  `notes-old`) can be refused with this message in a list that also follows a link; that fails closed.
- **The accepted limit (V-Q31 A, replacing V-Q30's "one look"):** in a list that sends one fence through
  a link into another inside a `follow` folder (R1, a fenced link plus a fence inside the folder it
  points to; R2, a fence written through a link into another fence), Voice may look at several folders'
  names, sizes and dates inside the other fence (one per folder down to the inner fence), and read where
  a link at or inside it points, the other fence's own link included when that fence is itself a link
  (V-Q31's "a fenced link can be read"), and then it refuses. The same holds when the home folder's own
  path runs into a fence, or through a fenced link written by another spelling: Voice looks, and may read
  that link, along the home folder's path before it finds the fence, then refuses. Never a file's
  contents, and the list never loads. Avoiding those looks would mean knowing where a fence written
  through a link really is before walking to it, which needs reading the fenced link first. Every case
  needs a fence list that routes one fence, or the home folder, into another fence through a link; no
  config we know of does. Tests pin each case.
- **Rule (b), outside that limit:** a fenced folder that is itself a link refuses the map, and Voice
  never reads that link, except in the cases the accepted limit above names.
- **Not covered here, filed separately:** a fence reached through a link into the `/System/Volumes/Data`
  spelling, or written in it, was fenced after the load only by that spelling; VF35 (below) now refuses it. A
  fence written with two leading slashes fences nothing (VF36). Both predate VF31.

## 2026-11-20: a fence or follow folder in a Mac alias spelling refuses the map (VF35)

- **The rule:** a `fenced` or `follow` value that starts with `/System/Volumes/Data` (any case), or whose first
  folder after `/` starts with a dot (`/.nofollow`, `/.resolve`, `/.vol`), refuses the compartment map (exit 2)
  before Voice looks at anything on disk. So does a fence whose path, once its links are followed, lands in either,
  and, when there is a fence, a home folder whose path, once its links are followed, lands in either. Each is a
  plain config error, never the fenced-link error, so no TODO item is added.
- **What the messages say:** a value written in an alias spelling: write its path without that start, or, if it
  has no path without it, move the folder into the home folder. A fence reached through a link into one: Voice can
  only fence a folder it can name without that start, so move the folder into the home folder or fence the folder
  the link stands in for. Neither message sends the reader to the other spelling.
- **Accepted cost (fails closed):** a folder that exists only on the data volume, such as stock macOS `/home`
  (`/home -> /System/Volumes/Data/home`), cannot be fenced by any spelling: written `/home/x`, it is reached through
  a link into the data volume; written `/System/Volumes/Data/home/x`, it starts with it. The fix is to move the
  folder into the home folder. Root links made by `synthetic.conf` meet the same refusal (reasoned, not run). Such
  fences are rare, and refusing is the safe side, like rule (b) for a fence that is a link.
- **Why refuse, not compare spellings:** on a Mac, `/System/Volumes/Data/<path>`, `/.nofollow/<path>` and
  `/.resolve/1/<path>` are all the same folder as `<path>` (checked: same device and inode), and Voice compares
  spellings. A plainly written fence catches a look by those spellings at or inside the fence, since its name
  appears inside theirs, but a fence written in one of them is never matched by a plain look. Refusing them means
  every fence that loads is plain, written and resolved. The dot rule covers the aliases rooted in a dot folder
  directly under `/` without listing them, since no real fence sits in one. The checks are by spelling only, so
  they run the same on every system.
- **Not covered here, filed separately:** a plain fence does not catch a look at a folder that holds it (an
  ancestor) written in an alias spelling or with `//`, so such a `--dir` can read a fenced file (VF37); a look
  written `/.vol/<dev>/<ino>` matches no spelling (VF38); a fence or link target with two leading slashes keeps its
  `//` (VF36). Those three predate VF35. Left as it is: a fence that holds the data volume (`/System/Volumes`)
  loads but does not fence the plain spellings inside it; no real fence holds a system folder.
- **What still needs the backstop's one-way match:** a link that enters an alias spelling inside a fence and climbs
  back out with `..` ends at a plain path, so this rule lets it through. The backstop still refuses that list for
  its look inside the fence. A test pins it.
