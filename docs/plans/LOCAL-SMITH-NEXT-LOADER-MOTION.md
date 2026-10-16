# LOCAL-SMITH-NEXT-LOADER-MOTION

## Bounded intent and authority

User request, 2026-10-15: give the Smith application intro loader an arbitrary minimum one-second hold with 0.5 seconds on either side, and a clean, professional presentation. This record binds that explicit request to local client presentation edits, related tests/documentation and local validation only, on the existing `feature/autonomy` worktree at base `7f6a483ae82c4bcf647268f035f1e7cc80409d4b`; scope expires at completion of this task. Existing unrelated uncommitted work must remain intact. No commit, branch change, installation, live service/model/provider action, publication or deployment is authorized.

Risk: elevated client/operator presentation flow, with no change to readiness, service lifecycle, protocols, persistence, observation bounds or authority. Source self-check and fresh independent assurance are required; neither is human approval for external effects.

## Independent inputs and reconciliation

Read-only independent producer, architect and designer contexts inspected the current Boot implementation before this plan. All found that `BootCoordinator` already owns readiness and `SmithShell` immediately mounts chat on ready. All recommended a separate UI-owned presentation gate, concurrent readiness work, lazy chat mounting, immediate repair, and no intro replay on ordinary rebuilds.

| Question / disagreement | Reconciled decision |
|---|---|
| One-second hold versus one-second total | Interpret the requested hold as **500ms intro + at least 1000ms fully visible + 500ms outro**, earliest successful handoff at two seconds. |
| Reduced motion: designer suggested a shorter static hold; producer/architect suggested preserving timing | Preserve the same two-second minimum but render fully opaque/static, without fades or a looping mark. Motion settings do not shorten or restart timing. |
| Fade mark only versus complete loading composition | Fade the compact loading composition over the unchanged theme background; keep its live semantics. Blocked/confirmation content always becomes immediately opaque/actionable, outside decorative waiting. |
| Repair / retries | Blocked/action-required outcomes interrupt presentation and satisfy the one-time minimum; subsequent observation/confirmed Start does not replay intro/hold. Successful recovery still exits over 500ms. |
| Scope of gates | Focused timing/safety tests, configured complete client gate and Linux compilation are source validation for this client-only change. Full product rebuild/release/installed graphical acceptance are not claimed. |

No unresolved blocking design disagreement. Task-local assumptions above expire with this implementation and remain reviewable by the operator.

## Traceability / implementation handoff

| Requirement / blocking acceptance criterion | Task | Evidence |
|---|---|---|
| LDR-01: fast readiness still shows 500/1000/500ms from first Boot frame; slow readiness extends hold without adding a second hold | TASK-01: UI-owned gate in `next/client/lib/boot.dart`, lazy shell handoff in `main.dart` | Explicit-duration app/widget tests at phase boundaries, held Core and synchronization futures |
| LDR-02: cosmetic time never establishes readiness or changes API effects, observation budget, Start confirmation, restoration or connection ownership | TASK-01/02: preserve coordinator and existing repair logic | Existing Boot/Start/unsafe-state tests, API order/count assertions, workspace/connection/exit regressions |
| LDR-03: reduced motion stays static on the same schedule; errors interrupt; disposal/rebuild cannot trigger stale or duplicate handoff | TASK-02: motion/lifecycle tests | Static painter/opacity/semantics tests, error/retry/retraction/rebuild/unmount checks |
| LDR-04: restrained theme-consistent vector presentation with accessible enlarged-text repair | TASK-01/02: lighter strokes, balanced spacing, no new assets/dependencies | Narrow/short enlarged-text layout tests and source-bound fake captures where available |
| LDR-05: document implemented timing without claiming deployment | TASK-03: update `docs/DESIGN.md` and record evidence here | Focused tests, formatting/analyzer, complete client gate, Linux build and fresh independent review |

Implementation order: gate and minimal visual refinement → deterministic focused tests → canonical Boot design update → self-check → independent assurance/adversarial review → record actual outcomes. Preserve any discovered unrelated failures rather than expanding scope.

Rollback: remove only this task's presentation gate/shell wiring, visual refinements, test additions and documentation hunks. No data migration or backend rollback is involved; never reset entire dirty files.

## Validation and review

**Implemented source, 2026-10-15:** UI-owned intro → hold → readiness wait → outro, using one preserved-duration animation controller with explicit phases. The hold begins on the first fully opaque frame, not the nominal intro deadline. No `BootCoordinator` or API behavior changed. The shell lazily mounts the existing conversation/restoration surface afterward. New vector strokes are 3px and mark/status spacing is 24px.

Initial self-check: the five-file focused Boot/restoration/connection/exit suite passed 67 tests; `make client/test` passed formatting, analyzer and 470 tests without skips; `flutter build linux --release --no-pub` passed. A fresh independent assurance context additionally ran the non-update visual comparison (2 tests) and three novel adversarial probes. Two passed, but one reproduced a medium defect: a delayed first opaque frame consumed the nominal combined intro/hold clock. That finding blocked acceptance and prompted explicit phase separation plus the retained delayed-frame test.

Remediation self-check: `flutter test --no-pub test/boot_test.dart --reporter expanded` passed 26 tests, including delayed-frame, fast/slow readiness, interruption, reduced-motion and disposal checks (`next/client/build/loader-remediation.log`). Visual fixture generation passed 2 tests; generation is not a comparison. Rendered intro/hold/outro and short/enlarged repair captures were opened and inspected. The short repair view is intentionally scrollable and its action is verified reachable.

### Final post-remediation results — 2026-10-15

| Check | Command / scenario | Result / evidence |
|---|---|---|
| Dependency resolution | `flutter pub get --enforce-lockfile --offline` from `next/client/`; configured gate also resolves the locked dependencies | PASS; no dependency/lockfile change |
| Focused Boot | `flutter test --no-pub test/boot_test.dart --reporter expanded` | PASS, 26 tests; `next/client/build/loader-remediation.log` |
| Configured complete client gate | `make client/test` from `next/` | PASS, 471 tests, no skips, formatting unchanged, analyzer clean; `next/client/build/loader-client-gate-final.log` |
| Linux compilation | `flutter build linux --release --no-pub` from `next/client/` | PASS; `next/client/build/loader-linux-build-final.log` |
| Visual comparison | Reviewer independently ran `flutter test --no-pub test/boot_visual_test.dart --reporter expanded`, without updating goldens | PASS, 2 tests; rendered frames inspected |
| Independent adversarial recheck | Delayed intro, endpoint/ticker ownership, late readiness retraction, rapid state/coordinator replacement, reduced-motion toggles, async synchronization failure/disposal | PASS, 9 novel in-memory probes; reviewer tool transcript |
| Whitespace / source stability | Scoped `git --no-pager diff --no-ext-diff --check`; SHA-256 comparison before/after final gates | PASS; all 11 recorded client/test/fixture inputs unchanged |

Fresh independent assurance recheck found **no open critical/high/medium/low scoped findings**, closed the delayed-frame defect, and returned **PASS for the source-only loader gate**. This is a recommendation, not human approval of external effects or unrelated worktree changes. The novel review probes were anonymous/in-memory; the core delayed-frame regression is retained in the repository.

Source identity: `next/client/build/loader-source-manifest.json` SHA-256 `43aea3c73f81e1c7e6eda4879891c98ae739469cfda00a68f70c034d8afb0b45`. Reviewed production files:

- `next/client/lib/boot.dart`: `17562b7e6f11a4964e116fbd26d60ddc3ec695cf4a4393896219d60783da97e0`.
- `next/client/lib/main.dart`: `396717fce16304b0a489724a86e75f53a7ef0f1fad0950742cef22a993e48271` (includes pre-existing unrelated work).
- Compiled `next/client/build/linux/x64/release/bundle/smith_client`: `c61dc7ad8a59082c3317a968d7fe9e84584f68f2791a93550674d687285ab236` (client compilation only, not a newly verified paired recipient package).

Logs/manifests are local ignored/generated evidence under `next/client/build/`, not durable release artifacts. Visual PNGs and tests are retained fixtures; installed graphical acceptance and production font-weight fidelity remain unverified. No installation, live service/model/provider action, commit, publication or deployment occurred. Source checks do not authorize release/deployment.
