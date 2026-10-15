# Installer appearance resources

**Implemented resource module, now integrated in the GTK GUI.** Bounded P0 T2 /
R1–AC1 work for approved **option A** in
[LOCAL-SMITH-NEXT-INSTALLER-REDESIGN](../../../docs/plans/LOCAL-SMITH-NEXT-INSTALLER-REDESIGN.md).
No approval, flow, worker, CLI, service, protocol or package behavior is changed
here. Only `src/appearance.rs` and this installer-owned assets tree were written;
client assets were read/copied without modification. No Cargo changes or new
Rust dependencies.

## GUI integration contract

`ui.rs` imports this module only in the GUI binary, loads fonts before widgets/worker construction, and uses the archive/trash images on labelled landing actions. The remaining check/X images are validated resources, not invented operation-status claims. Full OFL and icon notices are embedded and accessible through display-free `smith-installer-ui --licenses`. No separate recipient font asset or client-payload lookup is needed. Integrated results are recorded in `next/VALIDATION.md`; the resource-only evidence below remains historical.

Declare `pub mod appearance;` in the GUI binary (`src/ui.rs`), **not** the shared
library/CLI. The module is additionally gated by `feature = "gui"`.

```rust
pub const FONT_FAMILY: &str = "JetBrains Mono";
pub const ICON_SIZE: i32 = 24;
pub fn load_fonts() -> Result<FontAssets, String>;
pub enum Icon { Archive, Trash, Check, X }
impl Icon {
    pub fn from_name(name: &str) -> Option<Self>;
}
pub fn icon_image(icon: Icon) -> gtk::Image;
```

- Call `let _fonts = appearance::load_fonts()?;` in the GUI startup path **after
  help/version dispatch and GTK initialization, before CSS/widgets/text**, and
  retain `_fonts` through `gtk::main()` and widget teardown. Apply the family in
  parent-owned CSS, using actual weights 400, 500 and 600 rather than synthetic
  bold. Loading resources does not itself change CSS or create widgets.
- Registration is once per process. A static owner retains exactly three sealed,
  close-on-exec Linux memory files (824,852 font bytes total), even after a
  partial Fontconfig registration failure. An early memory-file creation failure
  closes its unregistered files. No stale/reused descriptor path can outlive its
  font bytes; no global font clearing, cache rebuilding, font install, payload
  lookup, subprocess or network request is performed. Repeated loads reuse the
  files and refresh the caller-thread Pango map. Registration failures are
  retained, not automatically retried. The OS reclaims memory/fds at process exit.
- `icon_image` requires the initialized GTK thread. It explicitly decodes embedded
  PNG, not SVG or an icon-theme/file lookup, and returns an intrinsically 24×24
  pixel image. These are fixed `#E3F0EE` strokes with transparency, **not symbolic
  CSS-tinted icons**. Parent owns visible action labels, accessible semantics,
  tooltip/focus behavior, contrast and ≥48px targets. Do not place these light
  icons on a light/cyan-filled button without an appropriate contrasting surface.
  Missing PNG support is a prerequisite failure (the infallible factory panics),
  not a stock-icon fallback. Decoder failure/dimensions are tested independently.
- `from_name` accepts exactly `archive`, `trash-2`, `check`, `x`; anything else is
  `None`. Neither the reviewed client subset nor the original supplied archive
  contains a download SVG. `Archive` is the explicitly disclosed package/install
  alternative, **not a Download alias**. Keep download actions label-first rather
  than inventing a glyph or silently changing provenance.
- In the existing **sole** GTK smoke test, after `gtk::init`, call
  `appearance::tests::assert_gtk_images()`. Do not add a separate GTK initialization
  test thread. The three ordinary resource tests are display-free and are picked
  up once the module is imported. Filter them with
  `cargo test --locked --features gui --bin smith-installer-ui appearance::tests -- --test-threads=1`.
- Keep GUI `--help`/`--version` ahead of the font/GTK path; the default CLI must
  never import this module or acquire GTK linkage. This resource-only change does
  not modify or claim to retest those parent dispatch paths.

### Native requirements

Supported prepared Ubuntu GTK3 already depends on Fontconfig, PangoCairo and
PangoFT2. The GUI module links those existing native libraries directly through
small private FFI declarations; builders need their development linker files
(already supplied by GTK3 development dependencies), recipients their shared
libraries and GTK's PNG decoder. Linux `memfd_create`, sealing and readable
`/proc/self/fd` are required. A sandbox denying those gives an explicit font-load
error, not a disk fallback. There is no font-path or resource-root override.

All runtime font/icon bytes are `include_bytes!`-embedded in the GUI ELF. No font
or icon path is resolved relative to cwd, the executable, the client payload or
an installed release. Uninstall and error presentation therefore do not depend
on candidate/client files. No recipient Python, SVG loader, renderer, `fc-cache`,
font installation or network is introduced. Normal GTK/system Fontconfig
initialization remains the existing native-library behavior; this module only
adds process-local application fonts and explicitly invalidates Pango's map.

## Provenance and notices

### Fonts

Unmodified static JetBrains Mono **v2.304**, copied from
`next/client/assets/fonts/jetbrains-mono/`, upstream commit
`cd5227bd1f61dff3bbd6c814ceaf7ffd95e947d9`:

- [Regular 400](https://raw.githubusercontent.com/JetBrains/JetBrainsMono/cd5227bd1f61dff3bbd6c814ceaf7ffd95e947d9/fonts/ttf/JetBrainsMono-Regular.ttf)
- [Medium 500](https://raw.githubusercontent.com/JetBrains/JetBrainsMono/cd5227bd1f61dff3bbd6c814ceaf7ffd95e947d9/fonts/ttf/JetBrainsMono-Medium.ttf)
- [SemiBold 600](https://raw.githubusercontent.com/JetBrains/JetBrainsMono/cd5227bd1f61dff3bbd6c814ceaf7ffd95e947d9/fonts/ttf/JetBrainsMono-SemiBold.ttf)
- [Upstream OFL](https://raw.githubusercontent.com/JetBrains/JetBrainsMono/cd5227bd1f61dff3bbd6c814ceaf7ffd95e947d9/OFL.txt), preserved at
  `fonts/jetbrains-mono/OFL.txt`, with copyright and the full SIL OFL 1.1 terms.

These links identify existing provenance; no network download was made for this
installer work. Binary hashes match the previously reviewed client copies.

**Implemented notice delivery:** `smith-installer-ui --licenses` exposes the full OFL/copyright and supplied-icon notice embedded in the installer independently of the client payload. The GUI help documents this display-free option. TTF metadata alone contains only a link/short notice; the separately embedded full text supplies the notice handoff. Icon redistribution permission is still unresolved.

### Icons

Read-only sources: `next/client/assets/icons/{archive,trash-2,check,x}.svg`.
Installer copies: `icons/source/`; generated runtime derivatives: `icons/*.png`.

Original operator-supplied archive: `smith-flutter-ui-redesign.zip`, SHA256
`c1ea52cebd00f860d61bd38b2fcad10234cdbfcde0fc4a8a6bbafc808a23a960`.
Only the exact named entries were read for verification; no archive instructions
were executed or other entries extracted. The normalized sources equal the
original entries with `currentColor` replaced by `#FFFFFF` and their white inline
style removed. Geometry, 24px viewBox, 1.8px round strokes are unchanged.

| Original ZIP entry | Original SHA256 (before client normalization) |
|---|---|
| `smith_ui_handoff/icons/archive.svg` | `82247472c3e41a45626774758f7fa3303ce72909eeefe2e74682bf9bbcbd62e0` |
| `smith_ui_handoff/icons/trash-2.svg` | `a4bad6304f23556ea1f71fc150997d00d04da97f46a656ad64961aef0b4f16e7` |
| `smith_ui_handoff/icons/check.svg` | `9c0a75e80808a1b681405636e64b2f0adfec89b23ac06340146fe5dbe4d3b49d` |
| `smith_ui_handoff/icons/x.svg` | `3db9ecdaf298426c44c36cbf738d04a40b4a372fa1a5a53c4b5c06492f0b02ff` |

**Supplied-icon redistribution rights remain unresolved.** See
[ICONS-NOTICE.txt](ICONS-NOTICE.txt). No upstream license is inferred, no rights
are granted by rasterization, and no publication/redistribution is authorized.

## Reproduction and checksums

The four PNGs were generated once using developer-only `generate_icons.py` with
librsvg **2.58.0**, Cairo **1.18.0**, GdkPixbuf **2.42.10**, libpng **1.6.43**.
It verifies the exact reviewed SVG hashes, substitutes only stroke color, and
renders transparent 24×24 PNGs. There are no external references, images, scripts,
fonts or URLs in these four simple SVGs. Python and librsvg are **developer-only**
regeneration tools, not build hooks or recipient dependencies.

From `next/installer`, optional explicit regeneration:

```sh
python3 assets/generate_icons.py
```

From `next/installer/assets`, verification (also covers normalized sources,
fonts, full OFL, icon notice and generator):

```sh
sha256sum --check SHA256SUMS
```

Different renderer versions may yield different PNG bytes; update hashes and
review derivative changes explicitly, never silently regenerate during a build.
See [RESOURCE-VALIDATION.md](RESOURCE-VALIDATION.md) for observed self-checks and
unrun parent integration/visual/release gates.
