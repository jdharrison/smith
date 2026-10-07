# LOCAL-SMITH-INSTALLED-MODEL-QA-FIX

## Intent and bounded authority

Operator reported installed local gpt-oss:20b visible but awkwardly formatted, with download/Continue confusion. Approved local diagnosis and focused UX bug fix under desktop-wizard scope; no model pull/inference, listener stop, service mutation, broad permission changes, publication or commits. Read-only fixed-loopback tags/health and process identification are diagnostic evidence only.

## Observed cause / reconciliation

Read-only Ollama /api/tags shows gpt-oss:20b exact local tag, valid digest/size and no remote/cloud metadata. Port 4783 is occupied by a Smith listener exposing legacy health (no chat-only profile/provider/model). Process inspection identifies plain smith serve parented by bash, not smith.service. Blocking a replacement backend is correct; the UI conflates this listener conflict with model provisioning and presents raw byte counts. No inference or mutation was performed.

Retain prior independent producer/architect/designer constraints: installed models require no pull; downloads are optional explicitly approved effects; incompatible listeners cannot be adopted/stopped; state/disabled-action explanations must distinguish prerequisites from ownership conflicts. Focused implementation changes display/error wording, not capabilities or lifecycle authority.

## Implementation and review

Implemented exact selected-tag labels, human-readable inventory sizes, explicit no-download-needed copy, collapsed optional download, and differentiated occupied-port/legacy profile guidance. Installed selection and Review remain available independently of final Start eligibility; ownership/capability guards unchanged. Developer self-check reported 147 Flutter tests, clean analyzer and Linux release build. Fresh focused independent static assurance passed without blocking source findings. Coordinator validation/rebuild/staging evidence is retained under `target/installed-model-qa/20261006T043809Z/`; inspect actual logs/results rather than treating this location as a pass. Real listener stop/start/inference remain unperformed pending exact approval.

## Acceptance and write scope

- Local installed exact tag selection has a primary path to Review Start without any download action or approval.
- Model names remain legible; sizes use bounded human-readable formatting rather than long raw byte strings.
- Optional download is visually secondary/collapsed; selected installed model explicitly says no download required.
- Legacy listener conflict identifies occupied loopback bridge/profile and required operator action; it never implies downloading the model resolves the conflict.
- Start remains blocked until a fresh absence check, no stop/takeover or legacy fallback. Compatible external backends remain protected; busy/cleanup guards unchanged.
- Widget/controller fixtures cover installed gpt-oss:20b, multiple models, legacy listener, selection/Review while conflict, unknown availability and zero setup pull/serve launch/reset before explicit allowed action.

Client write scope: client/lib/home/desktop_boot_shell.dart, client/lib/bridge/desktop_backend.dart and relevant client/test only. Coordinator docs/DESIGN.md and this record plus validation/staging evidence. Validate offline Flutter analyzer/full tests/Linux build and fresh focused assurance. Never overwrite the existing QA copy or change a running app/backend; a fresh private QA copy may be prepared under the previously approved staging method, with source identity and original files preserved. Real backend start still requires explicit operator action. Stop of identified legacy listener requires separate exact user approval.
