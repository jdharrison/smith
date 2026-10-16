# Installer option A — synthetic GTK evidence

**Implemented-source self-check, not independent assurance, visual acceptance, a release, or installed-host evidence.** Work record: `docs/plans/LOCAL-SMITH-NEXT-INSTALLER-REDESIGN.md`, option A. Base: `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`, existing `feature/autonomy` branch with concurrent work.

## Current results

Only **`gdk1-reviewed/` and `gdk2-reviewed/`** are current captures. Each contains **141 PNGs**, per-PNG JSON, complete test stdout/stderr, environment, exit code, run record and source hashes. Earlier directories retain historical failure/success records, not current validation. `SHA256SUMS` covers the current reviewed evidence files and this index; `historical-final-SHA256SUMS` preserves the superseded `gdk1-final`/`gdk2-final` export identities.

This fixture-only rerun follows the parent's architecture review correction: Working now says **“Working on this step. Cancelling does not undo earlier changes.”** and Confirmation says **“Review the next action before approving it.”** The parent also added preconsent/confirmation assertions and changed label selection from native blue to teal. The reviewed source includes the updated installer/resource READMEs. No source, resource, capture-helper or shared Dagger file was edited during this rerun.

| Check | Actual result |
|---|---|
| Existing `sh scripts/test-ui.sh`, GDK scale 1 | PASS, exit 0; fmt, warning-denied locked GUI Clippy, 9 tests |
| Same recipe, GDK scale 2 | PASS, exit 0; fmt, warning-denied locked GUI Clippy, 9 tests |
| Sole GTK initialization test | Extended in place; all prior safety assertions retained |
| PNG signature / IHDR dimensions / byte length / SHA256 against sidecars | PASS, all 282 final PNGs |
| Logical size × actual GDK scale = physical PNG size | PASS, all 282 |
| Source hashes before/after each run and against current files; cross-run equality | PASS, identical source snapshots |
| Focused helper Python AST parse; `git diff --check` for `ui.rs` | PASS on reviewed snapshot |
| Key-image inspection | Corrected Working/Confirmation copy and teal selection visible; inspected minimum/enlarged, wide-short/wide, result and 2× samples |
| `sha256sum --check SHA256SUMS` | PASS for current reviewed evidence |
| Full paired build/export, package verification, full installer/service/client tests | **Run independently by parent; not executed or claimed by this capture-only rerun** |

Reviewed runs began at observed UTC `2026-10-15T02:29:58` (raw timestamps retained in `run.json`). Elapsed: 144.65s (GDK1), 151.52s (GDK2). GTK test-suite times: 22.44s and 29.86s respectively. Each terminal invocation had a 900000ms timeout; neither timed out.

Exact commands from `next/`:

```sh
DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain python3 installer/scripts/capture-ui.py --output installer/tests/fixtures/ui-redesign/gdk1-reviewed
DAGGER_NO_NAG=1 DAGGER_CLOUD_TOKEN= DAGGER_PROGRESS=plain python3 installer/scripts/capture-ui.py --output installer/tests/fixtures/ui-redesign/gdk2-reviewed --gdk-scale 2
```

For a rerun, choose a **new** output directory under `installer/tests/fixtures/ui-redesign`; the helper deliberately refuses to overwrite or mix existing captures.

The helper uses `dagger run` and a standard-library API client to add a tiny export wrapper to an **in-memory copy** of the existing Python Dagger module. The wrapper invokes `SmithNext._installer_ui_tests(...)` and the unchanged `sh scripts/test-ui.sh`, not a duplicated builder recipe. No shared `next/dagger/src/smith_next/main.py` or other pipeline file is edited. The shell script executes:

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked --features gui -- -D warnings
xvfb-run -a env GDK_BACKEND=x11 cargo test --locked --features gui --bin smith-installer-ui -- --test-threads=1
```

The capture extension adds only the explicit output directory and GDK scale settings. A failed gate still exports diagnostics/partial captures and returns failure. Engine/image/native-package/Cargo downloads are developer-container effects; no host package was installed, no host display/manager/model was used, and no recipient GUI was launched. The existing recipe runs as UID/GID 1000 in Ubuntu 24.04 (observed 24.04.5), Rust/Cargo 1.98, GTK 3.24.41, Pango 1.52.1, Cairo 1.18.0, GdkPixbuf 2.42.10 and Fontconfig 2.15.0. Dagger CLI/engine: v1.0.0-beta.14. Exact Xvfb/xauth versions are in `environment.txt`.

## Coverage and image interpretation

Both physical display scales run the complete matrix:

- 720×560 at 100% and 200% text;
- 480×420 at 100% and 200% text;
- 1024×420 wide/short;
- 1440×900 representative wide;
- 2560×1080 ultrawide, bounded 640px reading column.

Every geometry captures landing, checking, artifact review, long exact confirmation, model choice, actual-byte numeric progress, indeterminate progress, success and partial failure. Additional 720×560 captures cover separate Enable, Start, download, preference, inference, destructive model removal and unregister confirmations; cancelling/cancelled preservation; and Upgrade, Uninstall, Model check and Model removal results with reachable Manage models.

`text1`/`text2` mean GTK base font settings `Sans 10`/`Sans 20` at 96 DPI; the production CSS still selects the **embedded JetBrains Mono**, not Sans. The metadata records the actual Pango description (for example `JetBrains Mono Semi-Bold 16.485` at text1). GDK scale 2 doubles the physical PNG dimensions independently of text scaling. Thus `gdk2-reviewed/landing-480x420-text2-…png` is 960×840 pixels, not a 480px image stretched after capture.

These are **test-only, fake-session client-area renderings** using `Widget.draw` → Cairo `ImageSurface` → `gtk::gdk::pixbuf_get_from_surface` → `savev`. `SMITH_UI_CAPTURE_DIR` is consulted only inside `#[cfg(test)]`; no production demo/capture path exists. No window manager decorations or compositor are included.

Scrollable pages have overlapping `scroll00`, `scroll01`, … tiles, including the last scroll position. No consent or failure text is shortened to fit an image. The long confirmation includes literal markup-like characters, a long identity, preservation restrictions, original-request/expiry language and an end sentinel; the label is asserted byte-for-byte equal to the supplied synthetic effects. At 480×420/200% text, long confirmation takes 16 tiles: this is intentionally visible scrolling, not hidden facts.

Selected entry points:

- [Landing, 720×560](gdk1-reviewed/landing-720x560-text1-scroll00.png)
- [Landing, minimum size, 200% text](gdk1-reviewed/landing-480x420-text2-scroll00.png) and [last tile](gdk1-reviewed/landing-480x420-text2-scroll01.png)
- [Reviewed confirmation copy and teal selection](gdk1-reviewed/confirmation-exact-720x560-text1-scroll00.png)
- [Exact effects, minimum size, 200% text, final tile](gdk1-reviewed/confirmation-exact-480x420-text2-scroll15.png)
- [Model choice, minimum size, 200% text](gdk1-reviewed/model-480x420-text2-scroll00.png)
- [Actual-byte progress and corrected Working copy](gdk1-reviewed/progress-download-720x560-text1-scroll00.png)
- [Successful setup](gdk1-reviewed/success-720x560-text1-scroll00.png)
- [Partial failure, minimum size, 200% text](gdk1-reviewed/partial-failure-480x420-text2-scroll00.png)
- [Wide-short landing](gdk1-reviewed/landing-1024x420-text1-scroll00.png)
- [Representative wide landing](gdk1-reviewed/landing-1440x900-text1-scroll00.png)
- [Ultrawide bounded column](gdk1-reviewed/landing-2560x1080-text1-scroll00.png)
- [2× display-scale landing](gdk2-reviewed/landing-720x560-text1-scroll00.png)
- [2× display scale, 200% text, exact-effects end](gdk2-reviewed/confirmation-exact-480x420-text2-scroll15.png)

Key-image inspection covered landing, default confirmation, numeric progress, minimum-size 200% confirmation (first/last tiles), model, partial failure, successful setup, 1024×420 and 1440×900 landing, and the 2× display-scale landing/exact-effects end. The two corrected sentences are visible in the default-size captures; selected text has a teal rather than blue background. Footer actions remain entirely in-window in the inspected enlarged-text samples, and the final exact-effects sentinel is visible. Minimum-size pages intentionally clip at the scroll viewport and require scrolling; this is not an assertion that all facts fit simultaneously. No new failure requiring a source edit was found. This inspection is developer self-check, not human visual acceptance.

## Assertions and UI-only fixes

The new assertions supplement, rather than replace, existing smoke scenarios:

- exact requested window dimensions; scroll viewport stays in-window and at least 80px tall;
- every visible footer action is wholly in-window and at least 48px tall; model chooser/removal controls also at least 48px; landing actions at least 88px;
- body allocations are horizontally contained, wrapped labels are not vertically clipped/ellipsized, and the complete scroll range remains reachable;
- large windows retain the bounded reading column; stack transitions remain disabled;
- real version/brand, embedded font family and embedded PNG icons;
- no attached normal diagnostics, no disabled pseudo-primary while working, no premature Close before worker join;
- no commands from opening/selection/resize/draw/scroll; request-bound explicit approval remains single-use; new confirmations have no default approval and reset focus to exact effects;
- repeated default/focused activation does not approve the next request; explicit button activation is still request-bound;
- Close sends no consent; completed/uncertain/failed effect rows and scope-specific result headings remain truthful.

Layout defects exposed and fixed **only in `ui.rs` widget construction/layout**:

1. Word-character wrapping on header/action labels created pathological one-character minimum-width height requests under enlarged text. Short header/action copy now wraps by words.
2. Catalog combo rows and the removal control imposed oversized single-line widths. They now wrap; chooser/removal controls have explicit minimum 48px targets.
3. An initially shown, subsequently hidden footer could affect the first window requisition. It stays hidden until the phase renderer explicitly shows it; its child widgets are initialized separately.
4. Changing footer orientation/body margins during GTK allocation could lose the resize invalidation, clipping a stacked footer or leaving the wide column unbounded. Geometry changes are deferred to a GTK idle callback.

The original test/layout subtask did not edit worker, protocol, consent dispatch, cancellation semantics, resources, CLI, multi-chat tests, client/service sources or shared Dagger code. The parent owns the resources and subsequent copy/CSS review corrections. This reviewed rerun changes **fixture evidence only**, leaving `ui.rs`, the capture helper and all shared source untouched.

## Source identities

Full installer/service input and asset hashes are in each final `source-sha256.json`.

| Input | SHA256 |
|---|---|
| Parent-reviewed `next/installer/src/ui.rs` | `3b8271c1a2a4a9b67eb719217a6fbb1ed6546053ff9ed76b7d54cad3658a62af` |
| `next/installer/scripts/capture-ui.py` | `25efe42f57a62b50507a1cc83e43310664039e30bfb744709f8cd7d44b2e6f08` |
| Parent-owned `next/installer/src/appearance.rs` | `3600a3378b5c04139139cf0973231f1220e092f78b8dbdc5e3125f2a5a50727d` |
| Parent-updated `next/installer/README.md` | `c33acc77ec03b88cc99f83cd821c05494d6d520f23ec0b9f50270a8c42d05d93` |
| Parent-updated `next/installer/assets/README.md` | `e3b2c5fd9605477157ad8763d9eb58da0d55580cc1113e76b1f1ed41ea9b04b7` |
| Unchanged `next/dagger/src/smith_next/main.py` | `d0c11a84199e152274ffbeb46f8c8d89b98c57da6126b6bc810a69720d3c7514` |

## Failures, superseded runs and gaps

Historical run records include original source hashes, environment, exit and stdio. Earlier redundant intermediate screenshots/sidecars were pruned as documented by their `retention.json`. For the now-superseded `gdk1-final`/`gdk2-final` sets, only PNG bytes were pruned: **all per-PNG JSON metadata remains**, together with `historical-final-SHA256SUMS` and new retention records. Their original source/run records were not rewritten. Representative earlier `FAILED-*` geometry images remain.

| Attempt | Observed result / resolution |
|---|---|
| Initial tooling discovery | Host Python had no SDK. Direct use of the module runtime hit SDK host/typed-ID/schema differences; replaced by the working in-memory normal-module export wrapper. No host package installation or shared-module edit. |
| Initial test compilation | Two GTK API mismatches (`Label.layout()` returns `Option`; window getter is `focused_widget`) fixed. |
| Initial geometry assertion | Hidden stack children are visible-flagged but unrealized; coordinate assertions now correctly inspect mapped descendants. |
| `run-01`, `run-02` | FAIL: 720×560/200% confirmation grew to 720×894. Word wrapping fixed pathological action-label requisition. |
| `run-03` | FAIL: 720×560/200% model page grew to 1844px wide. Wrapped model rows/removal copy fixed it. |
| `run-04` | FAIL: 720×560/200% result grew to 563px high. Header word wrapping fixed the remaining minimum-request problem. |
| `run-05` | FAIL: minimum-size 200% footer primary extended below the window. Deferred footer reflow fixed it. |
| `run-06` | FAIL: wide reading column remained unbounded. Deferred body geometry fixed it. |
| `run-07` | PASS, 128 captures, before additional approval/result fixtures. Superseded. |
| `gdk1`, `gdk2` | PASS, 141 captures each, before explicit model-control 48px minima/assertions. Superseded. |
| `gdk1-final`, `gdk2-final` | PASS, 141 captures each, before the parent's preconsent-copy/selection-CSS review fixes. Superseded; not current validation. |
| `gdk1-reviewed`, `gdk2-reviewed` | PASS, 141 captures each, matching current reviewed source snapshot; no new failures. |

The early tooling/compile/assertion attempts before `run-01` are documented from terminal results; their full raw transcripts were not exported. Do not treat historical passes as current evidence.

Remaining gates/gaps:

- Human visual acceptance and fresh independent assurance belong to the parent. The reviewed images visibly use the parent's teal selection CSS, closing the earlier native-blue observation. Images remain unretouched; this is not a claim of pixel-perfect Flutter parity.
- Native key-repeat/compositor behavior, window decorations, screen-reader/AT-SPI output, the opened native chooser popup, and fractional display scaling were not manually validated. Repeated GTK activation assertions are not a real desktop keyboard trial.
- No pixel-diff golden threshold or universal theme/font rendering guarantee is asserted. These are inspectable fixtures with geometry/content/resource assertions.
- No real manager, GPU, model transfer, inference, installed upgrade/uninstall or deployment. Synthetic effect strings test disclosure rendering, not real worker effect generation.
- Full installer/service/client tests, paired artifact build/export and package verification run independently under the parent. This capture-only rerun makes no claim about their outcomes. No commit, publication or release approval follows from this self-check.
