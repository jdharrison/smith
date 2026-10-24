# README-linked documentation cleanup


**Approved:** concise README and directly linked product guides, followed by explicit operator approval to commit and push the documentation cleanup straight to main. No application code, installed-host effects, release or Projects feature publication is included.

## Scope

README under200 words; installer, Scope, Architecture, Host requirements and Validation at most500 words each. License, Security and agent instructions unchanged. Complete detailed requirements and validation history remain in linked detail pages; summaries do not override them. Preserve links, section anchors, safety requirements and implemented/target/evidence distinctions.

## Publication binding

Base main `0d5f58bb756c907fa7cd23406afdf8a7057d43e0`. Build this docs-only candidate in a detached checkout, preserving the live worktree and index. Detailed pages retain exact main-version originals; validation only rebases relative Markdown links. Remove uncommitted Projects descriptions and disk5 claims from the published summaries; retain the original Projects-aware drafts locally. Publish only these eleven documentation files by normal fast-forward push; never force-push. Approval expires on scope/base drift.

## Validation

Check word counts, local links and incoming anchors, exact detailed-content retention, source unchanged, whitespace and independent final review. No application tests/builds are necessary for this docs-only change. Hosted CI and post-push verification remain distinct from local documentation review.
## Earlier local Projects-aware draft

The following records the separate local draft, not the published main-only candidate above.

**Approved local documentation scope:** operator requests concise documentation linked directly from root README, at most500 words per guide (README under200). No source, policy, approval, runtime, installation or publication change is authorized. No commit/push is included.

## Plan

Shorten `docs/SCOPE.md`, `docs/ARCHITECTURE.md`, `docs/HOST_PROFILE.md` and root `VALIDATION.md`. README and installer guide already meet their limits. License, Security and agent instructions are not product guides and remain unchanged.

Retain complete current requirements, implementation limits, uncommitted Projects additions and historical validation records in adjacent `*_DETAILS.md` references. These references remain authoritative for their detailed requirements; concise entry pages do not override them. Validation relocation must preserve content except relative-link repair. Preserve existing incoming section links through matching headings or direct detailed-reference links. Do not modify plans/evidence records or unrelated concurrent source work.

## Acceptance

Each selected guide has at most500 whitespace-separated words. Links resolve; existing relevant anchors remain reachable. Detailed source content is preserved, historical failures remain historical, and implemented versus target versus verified remains explicit. Run documentation checks and obtain independent review. Builds are unnecessary for documentation-only changes. Stop before commit/push.

## Outcome

Completed. Word counts: Scope386, Architecture440, Host439, Validation374; prior README173 and installer guide415 remain within limits. Independent review verified preservation against the recorded Projects supporting-input hashes, local links, inbound anchors, explicit detailed-policy delegation and implemented/target distinctions. It independently corroborated the hosted CI/PR merge statement through read-only GitHub queries. No blocking documentation finding remains.

The two pre-existing missing Projects results/review files remain absent and explicitly disclosed, not newly accepted evidence. Detailed validation content is preserved except relative-link rebasing. No application builds/tests, source edits, commits, push, installation or release occurred in this cleanup.

