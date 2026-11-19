# VF31 design check: a member's follow list (folders whose links Voice may follow)

Not a Codex brief yet. The owner answered V-Q29 with a new rule instead of A, B or C: a member's config names the
folders whose links Voice may follow (shared roots such as a projects folder or a wiki); every project stays its own
area; the rule is the member's own config, never a folder layout built into Voice. This note checks the rule against
the four leftovers (R1 to R4) from fix-up 2 (`621c61f`), says what it changes in V-Q28 (b), and lists what has to be
decided before fix-up 3. Fixtures below are invented.

## The rule, as it would be built

- The compartment map gains an optional `follow`: a list of folders (`~/` or absolute, no `..`, no fence inside
  one and no follow folder inside a fence).
- At load, `refuse_fence_links` walks each fence's path one part at a time, as today. **Below the fences' common
  folder** (fix-up 2's line; above it sit the system's own links, such as `/tmp` on macOS), a link is read only
  when it sits at or below a `follow` folder. Any other link refuses the map **before Voice reads it**, with a
  message naming the link and the two fixes (add its folder to `follow`, or write the fence by its real path).
- With no `follow` key, no link below the common folder is followed: the strict default. No known member config
  changes: no fence path crosses a link today.
- (b) is unchanged: a fence that is itself a link refuses the map, wherever it sits, and Voice never reads that link.

## Against the leftovers

| Leftover | Example | Closed by the follow list? |
|---|---|---|
| R1 | `~/secret` is a link to `~/real`, and `~/real/inner` is fenced too | **No.** The walk to `~/real/inner` crosses no link, so the rule never fires; its one look lands inside `~/secret`'s real folder whenever it runs before `~/secret`'s own look. Only reading `~/secret`'s link first would avoid it, and (b) rules that out. |
| R2 (M1) | `~/t/s/x/y`, and `~/p/q/al/s` with `al` a link to `~/t` | **Not in every order.** If the second walk runs first and `~/p/q` is not a follow folder, the map is refused at `al` with no look. If the first walk runs first, it crosses no link and looks at `~/t/s/x`, inside the second fence's real folder, before `al` is reached; which walk is safe to run first can't be known without reading `al`. |
| R3 (M2) | `~/secret` and `~/alias/inner/deep`, both `~/secret` and `~/alias` links to `~/real` | **Yes**, when `~` is not a follow folder: the walk meets `~/alias` and refuses before reading it, so `~/real/inner` is never looked at. |
| R4 | a fence whose own path loops back through a link inside it | **Yes**, when the link's folder is not a follow folder: refused before the link is read. |

Why R1 and R2 stay: in both, the look inside a fence happens on a walk that crosses no link, so no rule about
links can stop it. The walk would have to know where another fence's link points before that link is reached,
and reading a fence's own link is exactly what (b) forbids. Both need a fence list that contradicts itself (a
fenced link plus a fence inside its target, or a fence spelled through a link into another fence), and both are
still refused (exit 2); what leaks is one look at a folder's details, never its files.

## What it changes

- **In (b):** nothing for fences that are links. It narrows which other links Voice follows on a fence's path.
- **Regrouping:** not needed. A follow folder is any folder the member names.
- **Compartments:** the owner's example (a shared projects folder and a wiki, each project its own area) is mostly
  about how Voice labels a folder reached through a link, not about fences. Today every link is followed and a
  folder takes the compartment of where the link points. A follow list could decide that too: inside a follow
  folder a link takes its target's compartment; anywhere else it takes the compartment of where it sits (the rule
  VF30 already uses for linked queue folders). That touches every labelling path, so it is its own card if wanted.

## Open, for the owner (V-Q30)

1. For fences: the follow list as above, with R1 and R2 recorded and pinned by tests as "refused, one look"; or the
   follow list plus reading a fence's own link when it sits inside a follow folder (closes R1 there, but reopens (b)
   inside follow folders).
2. For compartments: a separate card that applies the follow list to labelling, or not now.
