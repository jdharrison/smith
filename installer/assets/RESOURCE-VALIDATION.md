# Installer resource self-check evidence

**Implemented module/assets only; not integrated UI, independent assurance,
release or deployment evidence.** Work record: approved option A / P0 T2 in
`docs/plans/LOCAL-SMITH-NEXT-INSTALLER-REDESIGN.md`.

Observed 2026-10-15 UTC (the implementation plan/operator date is 2026-10-14).
Base `2cc71a94fe924fbb9d324f29332e9f24c8c5991c`, existing branch
`feature/autonomy`, concurrent unrelated uncommitted work preserved.

- `src/appearance.rs` SHA256:
  `3600a3378b5c04139139cf0973231f1220e092f78b8dbdc5e3125f2a5a50727d`
- `assets/SHA256SUMS` SHA256:
  `b1b5a7bf94fbee4c2d718acc3557ec8d4f21d6f4b92e6f447938adf12422d98a`
- Embedded resources: 824,852 TTF bytes + 1,448 PNG bytes = 826,300 bytes.
- Environment: Ubuntu 24.04.5 LTS x86_64, rustc 1.98.0
  (`88d9e12ae`, 2026-08-18), GTK3 3.24.41, Fontconfig 2.15.0,
  Pango 1.52.1, GdkPixbuf 2.42.10.
- No downloads, package installation, Dagger invocation, live display, service,
  model, recipient install/uninstall, commits or publication.

## Executed checks

| Check | Observed result |
|---|---|
| `rustfmt --edition 2021 --check src/appearance.rs` | PASS |
| Standalone normal-module and test-module Clippy/rustc metadata checks, `-D warnings`, declared MSRV 1.83 | PASS, no remaining warnings/errors |
| No-`gui` compilation, with **no GTK/libc extern arguments** | PASS; the entire module is feature-gated |
| Headless embedded-resource test binary, isolated empty Fontconfig config | PASS: 3 tests, 0 failed/skipped; 0.04s final traced run |
| Same 3 tests with system Fontconfig, private HOME/XDG roots | PASS: 3 tests, 0 failed/skipped; 0.07s |
| Font content, seals, descriptor lifetime | Exact pinned TTF bytes; three sealed CLOEXEC memfds; writes/truncation rejected with EPERM; repeated load/drop/reload retains the same registration |
| Actual Pango font selection | 400/500/600 resolve to the respective retained `/proc/self/fd` files and actual Fontconfig weights 80/100/180, not installed fonts or synthetic weight |
| PNG decoding with external image loaders disabled | Four exact hashed transparent nonempty 24×24 RGBA images; unknown names rejected; no display initialization |
| Developer icon regeneration | Four PNGs reproduced byte-identically; reviewed normalized SVG hashes enforced; original ZIP normalization equivalence verified |
| `sha256sum --check SHA256SUMS` (assets directory) | PASS: all 14 entries; final source identity, generator syntax and equality to read-only client copies also passed |

Both traced runs used DISPLAY and WAYLAND_DISPLAY unset. The isolated run used
an empty font configuration (no system-font fallback can make it pass) and an
empty external GdkPixbuf loader manifest; the system-config run also disabled
external image loaders and directed HOME/XDG roots only to the owned temporary
test directory. These are test-process settings, not production environment
requirements or new production overrides.

### Observed syscall evidence

`strace -f -qq -e trace=%file,%network,memfd_create,fcntl,ftruncate,write,pwrite64`
wrapped the test process. Parsed isolated-run counts:

- `memfd_create`: **3**, `F_ADD_SEALS`: **3**;
- persistent write/create opens (`O_WRONLY`, `O_RDWR`, `O_CREAT`): **0**;
- mkdir/rename/unlink family mutations: **0**;
- socket/connect/sendto/recvfrom calls: **0**;
- librsvg / SVG image-loader opens: **0**.

The system-Fontconfig/private-HOME run also observed **3** memfds, **0** persistent
write/create opens and **0** network calls. This is observed resource-path
behavior on this host/configuration, not a claim that all native GTK/system
Fontconfig initialization on arbitrary hosts is filesystem-effect-free.
Normal test-output writes and font writes to anonymous memory files are expected.
Temporary binaries/metadata/configs/raw traces were kept only inside the new
assets subtree during checking and removed at handoff; no sensitive host paths
or raw traces are retained here.

## Exact focused compile method

No competing Cargo/Dagger build was started. Existing compiler-compatible cached
rlibs in `next/installer/target/debug/deps` were read; direct rustc/Clippy outputs
went only to temporary `assets/.validation/` files. From `next/installer`, the
linked test command was:

```sh
rustc --edition=2021 --crate-name smith_installer_appearance --test src/appearance.rs --cfg 'feature="gui"' -D warnings -L dependency=target/debug/deps --extern gtk=target/debug/deps/libgtk-98ce6f47b44c2d85.rlib --extern libc=target/debug/deps/liblibc-fbf8bc28fcf89111.rlib --extern sha2=target/debug/deps/libsha2-71ab3698e33ef019.rlib -o assets/.validation/appearance-tests
```

Both Clippy checks used the same extern/search paths and
`CARGO_PKG_RUST_VERSION=1.83 clippy-driver ... -D warnings`, with
`--crate-type=lib --emit=metadata` or `--test --emit=metadata`. The non-test check
omitted the test-only sha2 extern. The no-GUI check was:

```sh
rustc --edition=2021 --crate-name smith_installer_appearance_without_gui --crate-type=lib --emit=metadata src/appearance.rs -D warnings -o assets/.validation/appearance-without-gui.rmeta
```

Tests ran as `assets/.validation/appearance-tests --test-threads=1 --nocapture`
under the traced, private environment above. The temporary Fontconfig document
was `<fontconfig></fontconfig>`; its absolute test-owned path was supplied in
FONTCONFIG_FILE. GDK_PIXBUF_MODULE_FILE pointed to a comment-only temporary file.
This compiled factory/test-hook code but did **not** create a `gtk::Image` widget.

Initial checks caught a dead-code warning on the intentionally retained file
field; it was renamed `_files`, not removed. A raw Clippy-driver invocation
without Cargo's declared-MSRV environment suggested a newer `as_chunks` API;
rerunning with the real declared MSRV retained the 1.83-compatible code and
passed. These are final-source focused checks, **not** a locked workspace build
or proof that the code was compiled with Rust 1.83 itself.

## Parent-owned remaining gates

- Import the module only in the GUI binary, retain `FontAssets`, apply the family
  and real weights, use the selected icons with label-first accessible controls.
- Run `appearance::tests::assert_gtk_images()` from the existing sole GTK smoke
  test. Xvfb was not installed locally; no host display or alternative display
  server was used. Actual image-widget/layout/1×/2×/enlarged-text visual checks
  remain unrun, not skipped tests counted as PASS.
- Coordinated locked fmt/Clippy/test/build, isolated Xvfb, actual GUI no-display
  help/version, CLI no-GTK, and full paired-package checks on final integrated
  source. This resource-only work changed none of those entry points or scripts.
- Carry full OFL/copyright and supplied-icon notices in the distributable package
  or an easily viewable installer-owned notice surface; this write set does not
  change packaging or UI. Resolve supplied-icon redistribution rights before
  publication. No license is inferred from geometry or rasterization.
- Parent canonical-document/evidence reconciliation, fresh independent assurance
  and human visual acceptance. No self-approval or deployment authorization.
