# LOCAL-SMITH-NEXT-MINIMAL-CHAT

## Intent and scope

Explicit user task, 2026-10-13: remove setup/install from smith-app, which opens directly to minimal ChatGPT-inspired text chat, with no voice/speech. Local source edits and isolated validation on existing feature/autonomy worktree only; preserve all pre-existing work. No commits, remote actions, installed-host publication, service changes or weights. Elevated operator-flow scope. Base is the current uncommitted next candidate; historical validation is not evidence for this revision.

## Independent analyses and reconciliation

Architecture session `6bec97b4-29f1-4af4-aec8-c4f6d0248c02`; independent producer/designer session `0a8a870f-c25e-48e3-8af6-5f89dcd43da3`. Both support presentation-only removal conditional on readiness and recovery preservation. Producer/designer analyses share one fresh session, not separate reviewers.

| AC | Implementation | Evidence |
|---|---|---|
| AC1 Immediate chat, no install/model/voice controls | main.dart, chat_panel.dart | launch/absence matrix |
| AC2 Installer owns selection; no guessed model or configure | remove chooser, concise external guidance | missing model + external repair/Retry test |
| AC3 Preserve identity, history/draft/provisional answers, query-only Retry | controller/transport unchanged | existing safety regression suite |
| AC4 Independently scroll transcript, fixed multiline composer, neutral dark UI | bounded Column/Expanded transcript | long history geometry and narrow enlarged-text tests |
| AC5 Retain uncertainty/expired request and close-detaches semantics | existing confirmation, compact footer/status | existing recovery tests and review |

Blocking concern resolutions: missing model keeps Send disabled and directs external repair; readiness/identity inspection remains unchanged; expired-request confirmation is not provisioning and stays; no automatic submission, configuration, default choice, install or start. Transcript does not force-scroll while reading. No new dependencies/service/wire/storage changes. Shared configure API remains compatibility/test code, unreachable from app UI. Canonical Architecture/Design and client README updated without rewriting historical plans.

## Validation and rollback

Run client formatting/analyzer/tests with isolated compiled Rust fixture via make client/test, then make client/build. Record actual results in next/VALIDATION.md, obtain fresh assurance. This does not approve release or installed-host QA. Rollback is scoped restoration of this UI/docs/tests delta only, never wholesale worktree reset. Installer and installed candidate remain untouched.
