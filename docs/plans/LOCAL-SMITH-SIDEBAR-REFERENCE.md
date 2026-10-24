# LOCAL-SMITH-SIDEBAR-REFERENCE

## Bounded intent and authority

**LOCAL_PLAN_APPROVED:** the operator supplied the sidebar reference and explicitly requested that layout. Scoped local presentation, related tests/docs and isolated validation are authorized. Preserve the dirty `feature/autonomy` worktree, including all Agents/Projects/global-context changes. No commit, branch, remote mutation, install, application launch, data reset, service/provider/model/keyring effect or accepted-risk waiver follows.

This additive request supersedes only the 56px-wide desktop-rail/no-labelled-sidebar and rail-only-pixel-diff restrictions in [the original Dev plan](LOCAL-SMITH-DEV-NAVIGATION.md). Desktop width/reflow changes caused by the requested wider sidebar are intentional; unrelated content, theme, header, editor and behavior changes are excluded. Compact rail geometry and behavior remain unchanged.

## Independent inputs and planner reconciliation

- Producer `7b1b44b6-5fd2-4308-a0f4-fed2bfdc7311`: explicit labelled hierarchy and existing workflows; preserve no-effect/laziness/prefs; recognize necessary content reflow rather than claiming old rail-only diffs.
- Architect `13e58b0c-a398-4a02-b1dc-81093ba4ebe3`: preserve IconButton/focus/tooltip hooks; minimally replace one glyph-dependent Projects test finder; test editor breakpoints, compact/four-pane retention and semantics.
- Designer `70baf35b-7102-4e57-8d22-6d2996055e0b`: 184px base desktop width, top alignment, separate inline 48px chevron, guide/indent, root-only highlight and existing palette/type; grow label width for enlarged text. Built-in code/cube/robot glyphs avoid new assets.
- Planner selects those bounds: use 184px minimum desktop width with measured label-fit expansion, unchanged existing compact threshold/56px rail/modal, separate Dev root and disclosure. Use a downward expanded / rightward collapsed desktop chevron, indented Projects/Agents with decorative guide, Chats label, existing colors/fonts and built-in Material glyphs. No selected-child-as-selected-root semantics or forced expansion. Geometry-driven shell golden changes are permitted, not unrelated editor reference rebases.

## Requirements and acceptance

| ID | Contract | Evidence |
|---|---|---|
| SIDE01 | Desktop labelled Chats / Dev → Projects, Agents / Settings; code/cube/robot, inline chevron, selected-root background and left accent, child guide | Bundled-font sidebar snapshots; geometry/glyph/label tests |
| SIDE02 | Separate >=48px root/disclosure, unique selected/expanded semantics, keyboard/focus return; collapsed initially per pane/session | Existing Dev tests plus focused geometry/semantics/keyboard tests |
| SIDE03 | Compact 56px rail/chooser and current resize/hiding, keyed lazy pages, drafts/scroll/uncertainty, no eager registry/API work | Dev/Agents/Projects/compact/retention/split/preference regressions |
| SIDE04 | Existing header, theme, editors, service/API/schema and prefs untouched; only sidebar and required content reflow | Focused diff/source hashes; old frozen editor PNG hashes unchanged; analyzer/format and fresh assurance |

## Ownership, validation and boundaries

Parent owns `client/lib/ui/smith_workspace.dart`, minimal existing navigation finder updates, directly affected shell/Dev golden references, canonical Design docs and new plan/evidence. A test agent owns only new `client/test/sidebar_reference_test.dart`. No other production file is authorized.

Pre-edit shell, Projects navigation test and golden bytes are retained in `target/sidebar-reference/baseline/`. Run focused new/existing navigation tests, preferences/restoration/session-only, bounded format/analyzer and directly affected bundled-font golden suites. Preserve initial mismatch/failure evidence and distinguish content reflow from unrelated visual edits. Run broader client/build checks when available; earlier combined blockers cannot be waived or reported fixed by sidebar tests. Fresh independent review follows implementation/self-check. Native installed graphics and release acceptance remain separate gates.

**Initial status: proposal reconciled; implementation/validation pending.**
