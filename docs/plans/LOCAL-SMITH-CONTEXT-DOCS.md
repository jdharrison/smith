# LOCAL-SMITH-CONTEXT-DOCS

## Scope

**Approved local documentation cleanup:** the operator approved continuing the context-size cleanup with “dont go crazy” and wrapping up. Owner: this documentation session; approval expires at completion or scope drift. Risk: low, editorial only. No runtime prompt, policy, safety requirement, source behavior, host action, commit, branch change or remote effect is included. Preserve unrelated and concurrent work.

## Plan and acceptance

1. Copy current `PROCESS.md` and `DESIGN.md` byte-for-byte to adjacent `*_DETAILS.md` files, following the existing overview/detail pattern. These retain authoritative detailed requirements, including current uncommitted additions.
2. Replace only the two entry pages with short orientation and section links. Preserve every existing heading anchor and the exact Process approval-boundaries section. Aim for at least 80% fewer combined entry-page bytes; summaries cannot override details.
3. Clarify task-scoped reading and matching detail maintenance in `AGENTS.md`. Extend existing CODEOWNER review routing to the two new detail files with the same owner; no approval rule changes.
4. Check preservation hashes, heading/link compatibility, exact approval text, whitespace and size reduction; obtain fresh independent documentation review. No application builds are needed for this documentation-only change.

## Proportionate-work follow-up

**New explicit local approval:** the operator requested “do whatever edits are needed” to reduce documentation overhead. Scope is `AGENTS.md`, Process entry/details and developer/assurance output guidance: routine work uses a conversational record and focused checks; consequential work reuses records/analysis; document changed contracts only. Exact approval boundaries, elevated triggers, required relevant gates and historical evidence remain intact. No runtime/host/remote effects, commits or cleanup. This changes administrative workflow, not safety authority; the earlier editorial-only scope does not authorize unrelated changes. Validation passed: scoped `git diff --check`, six-file whitespace/local-link check (54 links), and byte-identical Process approval boundaries plus AGENTS approval/elevated-rule text. Fresh focused policy review caught two unconditional role-template requirements; both were corrected and re-review found no remaining blocker. No application builds/tests, new evidence bundle or host effects were needed.

## Working-tree preservation baseline

| Original | Bytes | SHA-256 (expected unchanged detail copy) |
|---|---:|---|
| `docs/PROCESS.md` | 34507 | `acb60b2b8ba28f6da7c7652f75b3e77270460b769fdaa7e25312ffd94ed907bb` |
| `docs/DESIGN.md` | 38491 | `12174a50ebd6668cc15d96cb2f3dbe8c32c8c6317caafa1ae2506e84b56134d6` |

Baseline local Markdown link targets in both source pages and `AGENTS.md` exist. Historical plans/evidence and unrelated prior documentation cleanup records remain untouched.

## Validation

**Completed — scoped documentation checks passed.** Read-only Python checks verified both preservation hashes, all 27 Process and 25 Design headings, 74 local links (including 52 fragments), byte-identical approval-boundaries text, whitespace and unchanged owner routing. Combined entry pages decreased from 72,998 to 12,429 bytes (82.97%; approximately 18,250 to 3,107 tokens using bytes/4, not tokenizer measurements). Scoped `git --no-pager diff --check` passed.

Fresh independent assurance session `c5b6e277-0805-4896-967c-42b7511fad53` independently reproduced these checks and found no blocking or other documentation findings. Detailed requirements remain authoritative; context reduction does not omit applicable constraints. This is documentation assurance, not runtime or release acceptance. No application builds/tests, remote-link checks, runtime edits, host effects, commits or publication occurred. This record grants no publication or release approval.
