# LOCAL-SMITH-CHAT-LOADING-STYLING

## Approved scope

**LOCAL_PLAN_APPROVED:** operator requests clean compact minimal chat loading, one or two lines maximum instead of stacked thinking/loading layers. Pure client presentation on existing `feature/autonomy`; preserve other work. No service/controller/protocol/queue mutation, execution, deployment or commits. Approval expires at completion/material drift.

## Implementation

Render an active local-send status as one unboxed, accessible live-region text line (maximum two wrapped lines), without assistant message header, redundant note or hourglass. Suppress the routine composer pending/retry block only when that same active local request is represented and no error, stale/disconnected state, expired request, unavailable route or uncertainty needs attention. Inactive/failed/unknown status bubbles and recovery remain unchanged. Preserve route privacy facts, provisional answers, real queue intents and actions. Existing queue shelf remains distinct from routine loading; no queue scheduling change.

Acceptance: Sending/Queued/Thinking each appear once in a maximum-two-line status; normal pending has no redundant recovery block; failures/unknown/expired/stale still expose recovery. Check narrow/enlarged layouts, timestamp semantics, optimistic sends and recovery regressions, formatting/analyzer and fresh independent review. No new animation, spinner or golden refresh. Native graphical acceptance is not inferred from widget tests.

## Evidence

**Implemented source; focused checks and fresh source review PASS.** Commands from `client/`:

- `dart format lib/chat_panel.dart test/chat_presentation_test.dart test/message_timestamp_widget_test.dart`:3 files,0 changed.
- `flutter test --no-pub test/chat_presentation_test.dart test/message_timestamp_widget_test.dart test/optimistic_send_test.dart test/message_queue_widget_test.dart test/message_queue_ui_assurance_test.dart --reporter expanded`:71 passed, exit0. Includes Sending/Queued/Thinking at360×400 with1x/2x text, error/stale/disconnected/unknown/expired recovery, and existing request/queue/selection/scroll/timestamp regressions.
- `flutter analyze --no-pub`: no issues, exit0.

Logs retained under `docs/evidence/chat-loading-styling/`. Fresh independent read-only assurance found no scoped blocker in status presentation or recovery suppression; it inspected retained logs without independently rerunning checks. Routed readiness/uncertainty guards were source-reviewed, not new routed-widget coverage. Linux build/native graphical acceptance were not run for this cosmetic change. No external or installed effects occurred or are authorized.
