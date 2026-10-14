# LOCAL-SMITH-NEXT-CHAT-SIDEBAR

## Intent and scope

Explicit user task, 2026-10-13: add a scalable desktop sidebar to the current Smith client with only Chat present and selected. Local presentation-only source edits and isolated validation on the existing `feature/autonomy` worktree. Preserve all connection, history, draft, uncertainty, Retry and close-detach behavior. No placeholder destinations, service/wire/storage changes, installation, host effects, commits or remote actions.

## Contract

1. The conversation remains the initially and currently selected surface.
2. A narrow persistent left rail exposes one recognizable Chat icon with selected styling, tooltip and selected accessibility semantics.
3. Do not add nonfunctional feature/mode controls or imply unavailable capability.
4. Keep the content surface independently expandable and usable at the existing 360×800 and 800×600 enlarged-text validation sizes.
5. Reuse the current dark neutral theme and retain all existing status, Retry, transcript, composer and recovery behavior.

## Evidence

Update `next/client/lib/main.dart` and focused widget tests. Run Dart formatting, analyzer/focused widget tests, then the project client test/build gate. Update canonical operator design and `next/VALIDATION.md`. Source validation does not authorize host installation or release.
