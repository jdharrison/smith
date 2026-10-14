# LOCAL-SMITH-NEXT-VOICE-CHAT-UPGRADE

## Exact one-time human approval

Operator approved with **“go ahead”** immediately after the proposed stop/back-up/replace/install-client/start/validate sequence. Applies only during this execution session to the trusted Ubuntu invoking account and existing `smith-next.service`, not legacy Smith, autorun/lingering, provider deployment, models, speech capture, remote effects or publication. Repository `smith`, existing `feature/autonomy`, checkpoint `04b72f5`, reviewed uncommitted candidate bound by artifact digests below. No commits.

Approved installed artifact SHA-256: `8d23bbf8cbb5aca2817bf4ea766d578071595ee7b7af7c9e131dcab78d65783a`.
Approved new standalone/bundled artifact SHA-256: `ac53d23c6cff1f83b671c687f14e7eb20ed24f1fedf284cc0ad2c0b78a1ed99e`.
New client manifest: `30c840b131007a5f20f769e4e46e668ace6207f5787c95a0773dce7b82feb394`.
Independent source/artifact review: `LOCAL-SMITH-NEXT-VOICE-CHAT-EVIDENCE.json`; installer relocation review and byte-equivalence recorded in `next/VALIDATION.md`. Approval is not permission to rebuild/change candidate inputs or accept new risks.

## Effects and checks

1. Read-only preflight validates old/new bytes, owned single-link0755 binaries, safe namespace/root ancestors, exact existing0600 unit text and installed Core observations. Initial observation: binary/unit installed, runtime stopped, bridge unavailable, persisted setup true. No actual provider query.
2. Create a private backup namespace under effective XDG data `smith-next/backups/voice-chat-ac53d23c`; preserve binary, unit, existing Core/app state, desktop/shim and current relative selector identity as applicable. No deleting/replacing unknown backups.
3. `/usr/bin/systemctl --user --no-pager --no-ask-password stop smith-next.service`; verify inactive/dead MainPID0, exact expected FragmentPath, no drop-ins/transient overrides, and no service/app sockets before publication. Do not touch other units or stale/foreign sockets.
4. Recheck source/old destination/state identities; stage new binary in same filesystem with0755, fsync, replace only the captured known old binary, fsync parent. No general installer upgrade bypass is added.
5. `python3 dagger/install.py --client client/dist --yes` from `next/`, scoped to the reviewed manifest and already-approved effects; preserves earlier client release and installs matching selector/menu/shim. Unit/state remain unchanged.
6. Start only the existing unit explicitly; no enablement, daemon-reload/unit rewrite or lingering change. Validate new exact binary/helper identity, Core bridge, app state envelope/history/preferences/readiness, MainPID and enablement independence. Do not invoke app models/submit/transcribe/speak, providers or GUI.
7. Record sanitized hashes/relative effect paths, observations and exit results. If any check fails, stop and report actual completed effects/backup, do not delete data or assume rollback.

## Execution outcome — 2026-10-11

Approved sequence completed. Initial service was already stopped; explicit Stop and inactive/dead/PID0 validation succeeded. Private backup preserves old binary, Core state, unit, desktop/shim and old verified selector. Reviewed new binary was published by same-filesystem atomic exchange with displaced old object retained. Matching client installer succeeded. Explicit Start succeeded; Core ready/setup true and the app state bridge verified against managed MainPID. Unit bytes and Core-state hash unchanged; enablement still static. No model selected, history empty, no provider query or GUI launch. [Sanitized execution evidence](LOCAL-SMITH-NEXT-VOICE-CHAT-UPGRADE-EVIDENCE.json) records hashes, scenarios and private backup-result references. Approval consumed; model/provider/audio rollout remains separate.

## Rollback reference, not automatically approved execution

With desktop closed and the exact unit verified stopped, a separately approved rollback can publish the preserved previous binary, select the preserved old client release, restore launcher/shim if changed and explicitly start/validate the prior unit. New app data is preserved, not removed. Never roll back over unknown files or unverified running work. Old service ignores the separate new app-state file. Backup and earlier client releases are retained.

No model downloads, Compose containers, runtime provider servers, real audio, restart policy/autostart/linger changes or legacy migration are in this approval. Live all-four-mode provider/audio validation remains a later gate.
