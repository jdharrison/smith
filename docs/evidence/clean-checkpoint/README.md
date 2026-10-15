# Clean-checkpoint evidence index

**Implemented evidence hygiene only.** Work record and current-session approval boundary: [LOCAL-SMITH-NEXT-CLEAN-CHECKPOINT](../../plans/LOCAL-SMITH-NEXT-CLEAN-CHECKPOINT.md). Base: `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`, existing `feature/autonomy` branch. No tests/builds, staging, commit, deployment, live installer/service/model actions or independent source assurance were performed by this retention step.

## Initial retained content — immutable historical mapping

| Directory | Files | Selection |
|---|---:|---|
| `launch/` | 4 | Every raw log referenced by the installer-launch evidence record |
| `redesign/` | 72 | Every file in the installer-redesign evidence record's `logs` map: ELF metadata, gate/verification logs, probe source, probe JSON and stdout/stderr, including empty stderr files |
| `multi-chat/` | 8 | Preserved component/aggregate/build logs, including the renamed historical failed/second runs and an additional initial component diagnostic |
| `client/` | 3 | Combined validation log/summary and connection-review machine-readable results |

The initial **87 curated files total 6,803,699 bytes** after sanitization. Their original mapping remains unchanged; the additive final extension and current totals are described below. The client `connection-review-test-results.json` is **newline-delimited JSON**, not one JSON document; the original filename is intentionally retained.

The [manifest](manifest.json) has **93 entries**: the 87 curated files plus six existing GPU plan logs. Four GPU logs were sanitized in place only after byte-exact originals were copied into ignored `next/service/build/clean-checkpoint-originals/` and made owner-read-only. Two tiny release-build logs are unchanged. Every entry records original and sanitized repository-relative paths, SHA-256, lengths, original-retention location, format, and transformation counts.

All **89 selected historical digest references resolve**. Seven require a different retained-original path: three multi-chat records whose published filenames were later reused, and four GPU log records whose original bytes are now retained in ignored storage. The referenced multi-chat digests match the preserved failed/second-run aliases; no historical record was rewritten. The manifest's `missing_references` array is empty for this selected set. Client raw files and the extra initial multi-chat diagnostic have observed-original digests, not invented historical attestations.

## Reading original evidence correctly

Old JSON manifests, reviews and their hashes remain unchanged. Their log digests identify **original bytes**, not the sanitized file now occupying an old GPU path. Resolve a reference by its evidence path, reference pointer and expected digest in `manifest.json`, then read the corresponding `sanitized_path`. `retained_original_path` is a local repository-relative recovery location, not a promise that ignored originals exist in a clean clone. Original bytes and original SHA-256 remain available locally; no new checksum substitutes for an old one.

Redaction replaces home-directory paths, transient/user-runtime paths and Dagger trace locators with explicit markers and strips terminal escape/control sequences. Credential/private-key/header/userinfo/signed-URL patterns are also checked and redacted if present. Counts are categories only, never sensitive matched examples. Nonprivate system library/tool paths are retained because they are material ELF/environment evidence. JSON/NDJSON syntax is checked after transformation. Plain text, timestamps, test outcomes and failures are retained; sanitization does not turn any failed, incomplete or stale run into PASS. Read the per-entry transformations rather than assuming byte identity.

The initial [hygiene verification](../../plans/LOCAL-SMITH-NEXT-CLEAN-CHECKPOINT-HYGIENE.json) records that retention step's original/derivative integrity, privacy-pattern scans, size, file-scope/index checks and exceptions. It remains an immutable historical snapshot, not a current byte-count claim after later additions. Those scans are bounded pattern checks, not a comprehensive secret audit or proof of independent provenance.

## Final gate extension — 2026-10-15

The additive [final mapping](final-manifest.json) retains **31 further records** under `final/`, totaling **2,539,435 derivative bytes**, without changing `manifest.json`, historical evidence digests, ignored originals or the initial hygiene report. Current totals are **118 curated supporting files** and **121 bundle files including the two manifests and this index**, under 10 MB and below the 20,000,000-byte cap.

| Final group | Files | Retained evidence |
|---|---:|---|
| Final client | 8 | `clean-checkpoint-commit-final` full log, summary, command result, before/after source inventories, source/baseline comparisons and artifact inventory |
| Prior client/package | 11 | Prior full client log/result/summary, source inventories/comparison, artifact inventory, two Git status records, and package log/command result |
| Service cleanup | 3 | Cleanup summary, service command result and service log |
| Installer cleanup | 2 | Installer command result and full log |
| Client cleanup provenance | 7 | All six logs cited by the current golden README plus the historical pre-refresh README, without duplicating its images |

Each original is SHA-256/length-bound to its sanitized derivative. **31 selected original digest references match; no selected reference is missing.** The original Git status records contain NUL separators and are explicitly mapped to readable `.bin.json` derivatives. All source-inventory entry paths/hashes and artifact entries are preserved exactly. Byte-identical original records would share a retained file; the before/after source entry maps match, but their complete original files differ in timestamp/provenance metadata, so both are retained rather than falsely claiming whole-file identity. No source inventory is copied merely under a second name without its distinct original identity.

Observed retained results, **not tests rerun by this curator**:

- Latest client gate: **341 passed, zero failures/skips**, analyzer clean and Linux release build successful; retained full log contains `+341: All tests passed!` and the command result is exit 0.
- Final source inventories: **1,564 identical before/after entries**, including **1,167 identical `next/` input entries**. The final manifest records an independently specified canonical digest without replacing the producer's original digest recipe.
- Service cleanup: **169 passed**, one controlled ignored fixture. Installer cleanup: **259 native + 9 GTK passed**, two controlled ignored fixture entry points. Both command exit records are 0.
- Package gate: prior **41-test PASS**, exit 0. Final inventories independently match the prior service (39 entries), installer (998) and Dagger/package (10) input groups. This is retained evidence supporting reuse of that source-package gate after the later client-only fix, not a claim that package tests or recipient-package export ran again.
- The latest standalone client artifact inventory contains **91 files** whose current content matched at curation. It is not a fresh paired recipient-package export. The prior client cleanup's 340-test result and earlier failures remain separately retained, not relabelled as the final 341-test run.

The service cleanup summary reports 1,050 stable inputs, but no separate cleanup input inventory was present in that requested directory; the later final inventories do not independently reconstruct that earlier run. Independent reviewer verification of this final mapping remains required. Parent owns final `next/VALIDATION.md`/plan updates and approval binding. This retention extension edits only this index and adds final evidence; it performs no tests/builds, staging, commits, code/test/ignore or canonical-document changes.

## Deliberate nonduplication and limits

- Existing synthetic GTK screenshots, sidecars, source manifests and current/historical retention indexes remain in `next/installer/tests/fixtures/ui-redesign/`. None is duplicated here or modified by this step.
- Existing client golden baselines remain in `next/client/test/goldens/ui-redesign/`; failure PNGs remain local diagnostics, not new baselines.
- The 72 redesign files contain no PNG or other binary screenshot payloads. `redesign/probe.py` is retained historical validation input, not an executable entry point to run during hygiene.
- Earlier full UI-redesign Dagger transcripts and early GTK tooling transcripts were not retained, and explicitly pruned historical images remain unavailable as already documented. This bundle neither fabricates them nor claims universal evidence completeness.
- Source validation, stable-snapshot binding, independent review, `.gitignore` cleanup and any local checkpoint commit remain parent-owned. Historical scoped PASS records do not approve subsequent combined changes.
- Icon redistribution permission, full license/SBOM/provenance review, native graphical/accessibility QA, installed-host and real model/GPU validation, and release/deployment approvals remain separate. This is not production or installed-host acceptance.
