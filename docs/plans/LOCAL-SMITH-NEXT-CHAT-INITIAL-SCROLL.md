# LOCAL-SMITH-NEXT-CHAT-INITIAL-SCROLL

## Intent

Explicit user task, 2026-10-13: when Smith launches into an existing active conversation, show the newest content by scrolling the transcript to the bottom. Local presentation-only client change and isolated validation; no service, storage, submission, installation, model, host, commit or remote effects.

## Contract

- Perform at most one automatic transcript scroll per mounted Chat surface, after retained history or a provisional terminal answer first becomes available and layout is complete.
- Do not repeatedly force-scroll on subsequent controller notifications, polling, model inventory, status changes or new work while the operator may be reading.
- Empty conversations remain unchanged.
- Preserve independent transcript scrolling, anchored composer, draft/recovery state and query-only behavior.
- Validate long retained history starts at the bottom while later user scrolling remains stable.
