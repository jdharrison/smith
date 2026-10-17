# README-linked documentation cleanup

**Approved:** concise README and directly linked product guides, followed by explicit operator approval to commit and push the documentation cleanup straight to main. No application code, installed-host effects, release or Projects feature publication is included.

## Scope

README under200 words; installer, Scope, Architecture, Host requirements and Validation at most500 words each. License, Security and agent instructions unchanged. Complete detailed requirements and validation history remain in linked detail pages; summaries do not override them. Preserve links, section anchors, safety requirements and implemented/target/evidence distinctions.

## Publication binding

Base main `0d5f58bb756c907fa7cd23406afdf8a7057d43e0`. Build this docs-only candidate in a detached checkout, preserving the live worktree and index. Detailed pages retain exact main-version originals; validation only rebases relative Markdown links. Remove uncommitted Projects descriptions and disk5 claims from the published summaries; retain the original Projects-aware drafts locally. Publish only these eleven documentation files by normal fast-forward push; never force-push. Approval expires on scope/base drift.

## Validation

Check word counts, local links and incoming anchors, exact detailed-content retention, source unchanged, whitespace and independent final review. No application tests/builds are necessary for this docs-only change. Hosted CI and post-push verification remain distinct from local documentation review.
