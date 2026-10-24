# Dev navigation — targeted shell golden closure

**Implemented: navigation-only golden rebase. Self-check PASS for the requested shell/Dev visual suites; not combined acceptance or independent assurance.**

This additive record follows the parent's explicit approval to include the complete intentional Dev hierarchy in the existing56px rail, not only the new disclosure row. It supersedes only the **shell visual blocker** in the earlier `../RESULTS.md`. Earlier evidence, hashes, failure renders and limitations remain preserved as historical results. No production source or original test behavior was edited for this follow-up.

## Pixel proof and update method

- Inspected every failed expected/actual image using exact **RGBA any-channel pixel comparison**, with zero tolerance. Recorded independent per-fixture rail geometry from the existing Width56 navigation widget and the vertical extent of its unchanged containing Row. Offstage panes were excluded; mixed-grid fixtures include all four visible rails. Geometry is relative to the full-image capture at device-pixel-ratio1.
- Subtracted only the measured rail footprint union from the whole-image difference mask. The raster rectangles are half-open, clipped to the image; floor/ceil merely covers intersecting pixels for fractional layout boundaries. No wider horizontal mask, header, sidebar, chat, editor or content allowance was introduced.
- **All23 updated PNGs have zero changed pixels outside the rails**, across159,815 changed pixels. Every captured actual also matches its corresponding first-failure actual render pixel-for-pixel. Old expected hashes match the canonical baseline manifest.
- Wrote proof records and archived original PNG bytes **before** any replacement. Only after verifying each old/actual hash and zero-outside proof did the targeted copy workflow replace the entire expected PNG with the actual render. No `--update-goldens` sweep was used.
- First round:19 original failures. Clearing those comparisons reached three additional entrance frames that had not previously executed; clearing those reached the final desktop620ms comparison. Each later frame received the same independent proof before its update:19+3+1=23 images. The unchanged desktop000ms image was inspected but never rewritten.
- Evidence-only wrappers under `target/dev-navigation/ui/visual-rebase/` import the unchanged original suites and install an instrumented comparator. It records geometry/raw image pairs and performs the same exact comparison, preserving failures and abort behavior. It cannot update goldens. Final validation uses the original test files and normal comparator, not these wrappers.

## Updated PNG ledger

All paths below are under `client/test/goldens/`. Coordinates are half-open pixel bounding boxes. **Every row has outside-rail changed count0.** Full old/new SHA256 values, measured logical/raster rail rectangles, capture paths and initial-render equality counts are in [`final-golden-ledger.json`](final-golden-ledger.json).

| PNG | Changed pixels | Changed bbox |
|---|---:|---|
| `ui-redesign/fixture-archive-1440x900.png` | 6256 | `[11,64,64,892]` |
| `ui-redesign/fixture-archive-400x400-text2.png` | 60 | `[11,133,12,201]` |
| `ui-redesign/fixture-chat-1280x720.png` | 6076 | `[11,64,64,712]` |
| `ui-redesign/fixture-chat-1440x900-ibm.png` | 6256 | `[11,64,64,892]` |
| `ui-redesign/fixture-chat-1440x900.png` | 6256 | `[11,64,64,892]` |
| `ui-redesign/fixture-chat-400x400-text2.png` | 3986 | `[11,129,64,392]` |
| `ui-redesign/fixture-chat-400x400.png` | 5439 | `[11,64,64,392]` |
| `ui-redesign/fixture-chat-4200x2400.png` | 7756 | `[11,64,64,2392]` |
| `ui-redesign/fixture-history-400x400-text2.png` | 3682 | `[11,129,56,347]` |
| `ui-redesign/fixture-overflow-1440x900.png` | 6256 | `[11,64,64,892]` |
| `ui-redesign/fixture-overflow-400x400-text2.png` | 3682 | `[11,129,56,347]` |
| `ui-redesign/fixture-production-shell-1280x720.png` | 6076 | `[11,64,64,712]` |
| `ui-redesign/fixture-recovery-400x400-text2.png` | 3986 | `[11,129,64,392]` |
| `ui-redesign/fixture-recovery-details-400x400-text2.png` | 2490 | `[11,129,64,392]` |
| `ui-redesign/fixture-settings-1440x900.png` | 6090 | `[11,64,64,892]` |
| `ui-redesign/fixture-settings-400x400-text2.png` | 5537 | `[11,129,64,392]` |
| `workspace-entrance/compact-text2-360ms.png` | 5285 | `[12,136,65,400]` |
| `workspace-entrance/compact-text2-620ms.png` | 5285 | `[12,136,65,400]` |
| `workspace-entrance/desktop-160ms.png` | 7175 | `[11,66,65,991]` |
| `workspace-entrance/desktop-360ms.png` | 6463 | `[12,66,65,991]` |
| `workspace-entrance/desktop-620ms.png` | 6463 | `[12,66,65,991]` |
| `workspace-entrance/mixed-grid-360ms.png` | 24630 | `[12,66,1165,1391]` |
| `workspace-entrance/mixed-grid-620ms.png` | 24630 | `[12,66,1165,1391]` |

The mixed-grid bbox spans multiple columns, but the **mask is four disjoint56px rails**, not that broad bbox: `[9,66,65,720]`, `[1109,66,1165,720]`, `[9,737,65,1391]`, `[1109,737,1165,1391]`. All intervening content pixels are unchanged.

## Validation evidence

Command from `client`, via the unchanged cached-SDK recorder:

```text
flutter test --no-pub test/ui_redesign_golden_test.dart test/workspace_entrance_visual_test.dart test/dev_navigation_golden_test.dart --reporter expanded
```

Actual command uses `dart <cached-sdk>/bin/cache/flutter_tools.snapshot --no-version-check`; exact sanitized argument vectors and source identities are in the records.

| Check | Expected | Actual | Record under `docs/evidence/dev-navigation/ui/` |
|---|---|---|---|
| Original shell visual suites plus new Dev real-font fixtures, normal comparator | All comparisons and test assertions pass | **23 tests passed, zero failed/skipped, exit0** | `20261018T011349541461Z/` |
| Per-image exact pixel proof, all rounds | Zero differences outside measured rails | **PASS,23 images,159815 changed pixels,0 outside** | `visual-rebase/pixel-proof-before-update.json`, `pixel-proof-round2-before-update.json`, `pixel-proof-round3-before-update.json` |
| Frozen/editor/source audit | Exactly approved PNG deltas; original source/tests, editor images, prior artifacts unchanged | **PASS** | `visual-rebase/final-golden-ledger.json`, `full-baseline-rebase-audit.json` |

No source drift occurred during the final test run. New Dev fixtures themselves remain unchanged from the previous UI handoff. No backend, build, package, full-suite or native-host gate was rerun.

## Preserved intermediate attempts

- `20261018T010943984918Z/`: instrumented initial capture retains1 pass/19 failures, with new raw geometry/expected/actual records in `target/dev-navigation/ui/visual-rebase/captures-shell/` and `captures-entrance/`.
- Proof attempt001 assumed only failures would be captured; it stopped on the unchanged passing desktop000ms image missing from the original failure archive. No PNG/proof was written. Original script and summarized failure remain as `prove_rails_attempt001.py` and `proof-attempt-001.json`. Corrected attempt002 retains the passing image separately and succeeds. No assertion about unrelated pixels was removed or weakened.
- Editor `copy_path` could not resolve the ignored generated captures; all19 requests failed without effects. The guarded local copy script applied the individually proven replacements instead; `apply-attempt-001.*` and `targeted-update-results.json` record this.
- `20261018T011158561380Z/`: original suites plus Dev fixtures after round1,20 passes/3 failures, newly reached later entrance comparisons.
- `20261018T011221998963Z/`: round2 instrumented capture,3 preserved entrance failures. `prove-round2-attempt001.*` and `apply-round2-attempt001.*` record proof and3 targeted updates.
- `20261018T011313286173Z/`: round3 capture,2 passes/1 preserved failure at newly reached desktop620ms. `prove-round3-attempt001.*` and `apply-round3-attempt001.*` record its proof and single targeted update.
- Every command attempt uses a new evidence path. No prior failure log, raw capture, archive, source audit, result or hash binding was overwritten.

Original first failures remain in `target/dev-navigation/ui/shell-golden-initial-failures.tar.gz`. Additional first failures and exact original PNG bytes are retained in `visual-rebase/shell-failures-round2.tar.gz`, `shell-failures-round3.tar.gz`, `old-shell-pngs-before-update.tar.gz`, and the round2/3 old-PNG archives. The canonical baseline source archive remains unchanged.

## Scope/frozen audit and remaining blocks

- All1337 baseline paths were checked against the prior handoff plus the23 approved PNG changes: **zero unexpected non-doc changes**. The previous three UI-owned and four parallel-owned source/test deltas remain exactly as previously bound. No follow-up production or original test source change occurred.
- Exactly23 existing golden paths differ from baseline, all in the two named shell directories. **All13 Agents/Projects editor-only golden images remain frozen**. New Dev fixture hashes are unchanged.
- All54 artifacts bound in the original UI evidence package remain unchanged. New proof/ledger/artifact hashes are additive in `hash-bindings.json` in this directory; original bindings remain historical, not rewritten.
- The three previously reported Projects editor-only failures remain **unrelated label-change failures**, each865 pixels. Those images were not updated or rerun here. Agents editor-only images remain frozen; the prior Agents golden pass is not a new combined acceptance claim.
- Full client/Linux build, compiled/root/package/native graphical/IME/accessibility gates and fresh independent assurance remain with the parent. No accepted-risk decision, commit, Git mutation, remote action, installation, app launch or live-host effect occurred.

**Quality gate:** requested shell/Dev visual closure PASS; zero unrelated pixel changes; production/test behavior untouched. Existing Projects editor-only failures and parent combined/independent gates remain unresolved, not waived.
