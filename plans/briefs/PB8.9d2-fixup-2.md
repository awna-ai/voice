# PB8.9d2 fix-up 2: the recovery sentence shows once

Lane V, runner V1, 2026-11-17. Same rules and gate as fix-up 1. Test first. Don't commit.

The repeat Verifier's should-fix: after an invite that fails once Radio made the fleet, the error box ends with the
recovery sentence (the engine appends it) and the recovery alert shows it again, so it is read twice.

- In `builder/ui/src/screens/OtherComputers.vue` `makeCode()`'s `catch`: when `recoveryPath(error.value)` finds a
  path, set `recoveryFile` as now **and** remove that sentence from `error.value`
  (`error.value.replace(recoverySentence(path), '').trim()`), so the error box keeps only the failure and the alert
  holds the recovery sentence.
- In `OtherComputers.test.ts`, the test of an invite failure that carries the sentence goes back to asserting
  exactly: the error alert's text is the failure without the sentence, and the card's text holds
  `recoverySentence(path)` exactly once (count the occurrences in `wrapper.text()`). A mutation that drops the
  removal must fail it.

Gate: `pnpm -C builder/ui test`, `pnpm -C builder/ui run typecheck`.
