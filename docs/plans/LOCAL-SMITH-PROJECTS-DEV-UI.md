# LOCAL-SMITH-PROJECTS-DEV-UI

## Bounded intent and approval

Owner: coding agent; human requester approves scoped local UI edits and focused source checks via the explicit request and follow-up “go”. No commits, branch changes, remote mutation, installation, deployment or live runner effects are authorized.

## Requirements and acceptance

- REQ-1: Projects list header shows Projects on the left and a plain Create button on the right, without the plus icon; retain create eligibility, identity and draft behavior.
- REQ-2: Temporarily hide Saved agent role and its configuration hint in Dev, including focus/semantics and occupied layout space.
- REQ-3: Preserve runner/controller/service logic and agent admission requirements. Do not select a default agent or implement the WIP successor. Fresh-form Start remains disabled.

Risk: low, presentation-only. Exclusions: navigation, persistence, schemas, execution, model/provider behavior and host effects. Existing unrelated working-tree changes must be preserved.

## Implementation and evidence expectations

Change only the two view presentations, focused widget expectations and matching canonical design descriptions. Use the existing header semantics and button sizing. Retain the agent control behind non-maintained hidden Visibility so restoring it does not require rebuilding its logic.

Run targeted Dart formatting, Flutter analyzer and Projects simplification/Dev widget tests where available. Obtain fresh independent source review. These are source checks, not native graphical/runtime acceptance or release approval.

## Source-check outcomes

- `dart format lib/ui/dev_jobs_view.dart lib/ui/projects_view.dart test/projects_simplification_test.dart test/dev_jobs_test.dart` from `client/`: passed, four files checked, zero changes.
- `flutter test --no-pub test/projects_simplification_test.dart test/dev_jobs_test.dart` from `client/`: all 10 tests passed.
- `flutter analyze --no-pub lib/ui/projects_view.dart lib/ui/dev_jobs_view.dart test/projects_simplification_test.dart test/dev_jobs_test.dart` from `client/`: no issues found.
- `git diff --check`: passed for tracked changes; the reviewed source/test files are currently untracked, so this is not their diff validation.
- Fresh independent read-only assurance inspected the current source and independently reran formatting, analyzer and all 10 tests successfully. No blocking findings; native graphical/focus/screen-reader acceptance and broader build/package/runtime gates were not run. Explicit assertions for disabled Start after project selection and focus/semantics would strengthen coverage; current source retains those guards and default Visibility exclusion.

No service/controller/runtime changes or live-host effects were made by this task. Source checks are not release or installed acceptance.
