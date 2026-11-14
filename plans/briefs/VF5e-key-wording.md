# VF5e brief: Connect Claude's key text fits any computer

VF5b's new text on Connect Claude says "Mac" twice, and `builder/ui/src/wording.test.ts` (*uses words
that fit any computer*) fails on `screens/ConnectClaude.vue`: screens never name a kind of computer, and
the key field shows only where the backend says `consoleTakesKey`, so the screen needn't say where.
Same rules as VF5b's brief: public repo, made-up keys only, red first, **commit nothing**, and never
touch `.ai/`.

Read first: `builder/ui/src/screens/ConnectClaude.vue`, `ConnectClaude.test.ts`, `src/wording.test.ts`.
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`,
`~/.codex/skills/frontend/SKILL.md`.

## 1. Changes (wording decided; use it exactly)

1. `#console-key-help` becomes:
   "Make a key at console.anthropic.com, under API keys. Voice keeps it in this computer's password
   store and gives it only to Claude Code. What Claude Code does with it is billed to your Console
   account."
2. The `consoleTakesKey` footer paragraph becomes:
   "Voice never sees your password. It keeps your Anthropic Console API key in this computer's password
   store and gives it only to Claude Code. Its sign-in is separate from the Claude app and from the
   Claude Code you use yourself. Claude is the only choice for now. Other AI providers come later."
3. In `ConnectClaude.test.ts`, update `keyFooter` and the `#console-key-help` expectation to those
   exact sentences (red first: change the expectations, see them fail, then the screen).
4. The test *keeps Console sign-in available and shows the Mac refusal from the backend* mocks a
   backend sentence that no longer exists (VF5 removed that refusal). Rename it *keeps Console sign-in
   available and shows a refusal from the backend* and make its `message` a made-up one,
   `'Claude Code could not start the sign-in.'`; the assertions stay.

## 2. Checks

In `builder/ui`: `pnpm test --maxWorkers=2` (all green, `wording.test.ts` included) and
`pnpm typecheck`. Report the files changed and the exact commands with their results.
