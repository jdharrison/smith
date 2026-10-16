# Smith UI asset provenance

Implemented local assets; no runtime font or icon network requests.

Fonts are unmodified official static TTFs: JetBrains Mono v2.304 (commit
`cd5227bd1f61dff3bbd6c814ceaf7ffd95e947d9`) and IBM Plex v6.4.0 (commit
`383c681f015ed2626e919ec4e3cca16ccc204e9d`). Each includes upstream SIL Open
Font License 1.1 in its `OFL.txt`, including copyright/reserved-name notices.
Weights: Regular 400, Medium 500, SemiBold 600. Fonts are bundled, not installed.

Icons originate solely from the operator-provided `smith-flutter-ui-redesign.zip`,
SHA256 `c1ea52cebd00f860d61bd38b2fcad10234cdbfcde0fc4a8a6bbafc808a23a960`.
Only the named icon entries below were imported; no archive instructions or raster
reference were extracted. Geometry is unchanged; CSS currentColor is normalized
to literal white, then SmithIcon applies an explicit srcIn tint.
**Icon redistribution license was not supplied. Publication remains blocked until
rights are established; do not infer an upstream license from visual similarity.**

## Integration and packaging handoff

- `SmithTheme.dark({String fontFamily = 'JetBrains Mono'})` returns ThemeData;
  `SmithTheme.alternativeFontFamily` is `IBM Plex Mono`. `SmithColors` exposes
  semantic palette constants; `SmithTokens` is the geometry ThemeExtension.
- Muted text is lifted to `#789290` for 4.5:1 contrast on background, surface and
  raised surface. Selected rows should use primary/secondary text, not muted text.
- `SmithIcon(name, size: 24, color: ..., semanticLabel: ...)` only loads the
  allowlisted local SVGs. Null color inherits IconTheme; null label is decorative.
  Callers own interactive semantics, tooltips and 48px targets.
- `await BuildInfo.load()` returns immutable metadata; `.buildVersion` includes
  version and build number, or `Development build` when missing/malformed.
  The source is Flutter-generated `version.json`, not a hardcoded application
  version. Verified in Flutter 3.47.2 Linux `LinuxAssetBundle.build` /
  `getVersionInfo` and an existing release bundle. Linux asset assembly derives
  it from the project/pubspec (respecting Flutter build-name/number overrides).
  Widget-test asset bundles omit it. No build-helper changes were needed.
- `flutter_svg` is pinned to stable 2.2.3; all transitive versions and source
  hashes are locked in pubspec.lock. It uses vector_graphics/codec/compiler,
  path_parsing and XML/parser dependencies; HTTP/web dependencies are transitive
  library capabilities, not used by this local-asset wrapper. No native plugin or
  OS dependency was added. Locked resolution succeeded on Flutter 3.47.2 /
  Dart 3.13.2. This is compatibility evidence, not a dependency security audit.
- Package owner: include `data/flutter_assets/` followed by each exact `assets/`
  path listed below, plus `assets/README.md`. FontManifest and AssetManifest are
  Flutter-generated. Preserve `version.json` and generated dependency NOTICES.
  Do not loosen the package allowlist to accept arbitrary asset directories.

## Bundled file hashes and exact origins

- `assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf`
  - SHA256 `a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f`
  - https://raw.githubusercontent.com/JetBrains/JetBrainsMono/cd5227bd1f61dff3bbd6c814ceaf7ffd95e947d9/fonts/ttf/JetBrainsMono-Regular.ttf
- `assets/fonts/jetbrains-mono/JetBrainsMono-Medium.ttf`
  - SHA256 `31c92d01a8a08528b718a43addf0ad3df0af2ca4b7b3290a452f70f358e14d3d`
  - https://raw.githubusercontent.com/JetBrains/JetBrainsMono/cd5227bd1f61dff3bbd6c814ceaf7ffd95e947d9/fonts/ttf/JetBrainsMono-Medium.ttf
- `assets/fonts/jetbrains-mono/JetBrainsMono-SemiBold.ttf`
  - SHA256 `1b3bfa1ed5665a4ce3f9feb68d2d4e40e70bf8b4b7d9a3edd418f321b4e166a0`
  - https://raw.githubusercontent.com/JetBrains/JetBrainsMono/cd5227bd1f61dff3bbd6c814ceaf7ffd95e947d9/fonts/ttf/JetBrainsMono-SemiBold.ttf
- `assets/fonts/jetbrains-mono/OFL.txt`
  - SHA256 `30f0c136e3c88e422d0791acd97238870f9054a9729bc34cf2ff0d4ed8cac4ad`
  - https://raw.githubusercontent.com/JetBrains/JetBrainsMono/cd5227bd1f61dff3bbd6c814ceaf7ffd95e947d9/OFL.txt
- `assets/fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf`
  - SHA256 `fe11304a5fe956d5744e9b6a246cc83d90425245e75a62230044966ca96a7f50`
  - https://raw.githubusercontent.com/IBM/plex/383c681f015ed2626e919ec4e3cca16ccc204e9d/IBM-Plex-Mono/fonts/complete/ttf/IBMPlexMono-Regular.ttf
- `assets/fonts/ibm-plex-mono/IBMPlexMono-Medium.ttf`
  - SHA256 `0bede3debdea8488bbb927f8f0650d915073209734a67fe8cd5a3320b572511c`
  - https://raw.githubusercontent.com/IBM/plex/383c681f015ed2626e919ec4e3cca16ccc204e9d/IBM-Plex-Mono/fonts/complete/ttf/IBMPlexMono-Medium.ttf
- `assets/fonts/ibm-plex-mono/IBMPlexMono-SemiBold.ttf`
  - SHA256 `c9417148ce13f8fa7d2d5c9180bbc141f72aa0d814ffeb280f6904dc2b1bbd7a`
  - https://raw.githubusercontent.com/IBM/plex/383c681f015ed2626e919ec4e3cca16ccc204e9d/IBM-Plex-Mono/fonts/complete/ttf/IBMPlexMono-SemiBold.ttf
- `assets/fonts/ibm-plex-mono/OFL.txt`
  - SHA256 `91c25c350d3cac39da2736d74f7ba37ef648f5237a4e330a240615bc8d8c4360`
  - https://raw.githubusercontent.com/IBM/plex/383c681f015ed2626e919ec4e3cca16ccc204e9d/IBM-Plex-Mono/fonts/complete/ttf/license.txt
- `assets/icons/chevron-right.svg`
  - SHA256 `43b39bf125cee680dd474f2f0ba9c7bfc370df2087280cacfd5d62499a7cbb0b`
  - user ZIP: smith_ui_handoff/icons/chevron-right.svg
- `assets/icons/message-square.svg`
  - SHA256 `39ef163029f320935e3becc3e478393ce5ea4e4b917d4d8f222542eb5758f741`
  - user ZIP: smith_ui_handoff/icons/message-square.svg
- `assets/icons/settings.svg`
  - SHA256 `0b22e51750886c65688e044434a578191799ffcf4afea4cbf60fdf2e9ea90327`
  - user ZIP: smith_ui_handoff/icons/settings.svg
- `assets/icons/archive.svg`
  - SHA256 `ce0af5c71277130bdad501ed06249213e9805b0dd445e3da97af3bc6d32909ce`
  - user ZIP: smith_ui_handoff/icons/archive.svg
- `assets/icons/plus.svg`
  - SHA256 `6e1a6a826663c1a4fbd4fd8f8bf9d7765fa965744d4e1d6791f163ba378b2b09`
  - user ZIP: smith_ui_handoff/icons/plus.svg
- `assets/icons/ellipsis-vertical.svg`
  - SHA256 `af99c5d1a45eef4e3f59765b1440fe51f39169bb057473fdaf925666f57e8e5a`
  - user ZIP: smith_ui_handoff/icons/ellipsis-vertical.svg
- `assets/icons/pencil.svg`
  - SHA256 `6de88a293c49416488fb0c5c79177d7af7c855240391b99a561062498a179ab4`
  - user ZIP: smith_ui_handoff/icons/pencil.svg
- `assets/icons/trash-2.svg`
  - SHA256 `958240c9bacb4dca8f2ea66e08a817eab4ace45c9e72086472095edaa7cc3a9f`
  - user ZIP: smith_ui_handoff/icons/trash-2.svg
- `assets/icons/send.svg`
  - SHA256 `789dbefe721043a8fbb92c7eac0904c59cfab83b2faaa9ee924a53d9f6045933`
  - user ZIP: smith_ui_handoff/icons/send.svg
- `assets/icons/x.svg`
  - SHA256 `046d06a319a47b847e1053e557f1cddf3fad2f53abad2150e563cec9ad404d92`
  - user ZIP: smith_ui_handoff/icons/x.svg
- `assets/icons/menu.svg`
  - SHA256 `69a8310d92c0f2a42fd3127e50c2f7bebd87398d32c5def66db31a180e88f277`
  - user ZIP: smith_ui_handoff/icons/menu.svg
- `assets/icons/rotate-ccw.svg`
  - SHA256 `837375539874ab13b51911472d13a947582ff79cb5b926388b279d03914d64bb`
  - user ZIP: smith_ui_handoff/icons/rotate-ccw.svg
- `assets/icons/info.svg`
  - SHA256 `62505fa5145acd57a45fd93f49df02f1e0dc86d4c43c75a2ca69fabe8fe554b3`
  - user ZIP: smith_ui_handoff/icons/info.svg
- `assets/icons/check.svg`
  - SHA256 `f5914af9d3a5dc1e77ebf278af6488dc17b1935d96f50f1f8dae78e625b1c5b1`
  - user ZIP: smith_ui_handoff/icons/check.svg
- `assets/icons/chevron-down.svg`
  - SHA256 `640e640066191f6b26236c4c24f487ed9ba4493ba0d33b480f930825dffecfbc`
  - user ZIP: smith_ui_handoff/icons/chevron-down.svg
