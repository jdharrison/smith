# Workspace entrance visual fixtures

Synthetic captures of the real `SmithApp` composition with fake service/app APIs and an in-memory preference store. No installed service, model or provider is used. Saved workspace hydration is held independently of Boot, then released before the presentation clock starts.

- `desktop-*`: 1600×1000, at 0/160/360/620ms. The frame and final geometry remain fixed while header, rail/history, title, transcript and composer enter separately.
- `mixed-grid-*`: 2200×1400 restored 2×2 with Chat and Settings, at 360/620ms. All visible panes share one clock.
- `compact-text2-*`: 400×400 at 2× text, at 360/620ms. The existing minimum-height canvas remains scrollable rather than shrinking controls or clipping layout; the lower composer is reached by canvas scrolling.

The already-bundled MaterialIcons and JetBrains Mono Regular fonts are explicitly loaded. Regular glyph captures do not verify production weight selection. Build/version labels and conversation/service observations are synthetic fixture state, not proof of an installed release. PNGs demonstrate sampled progress, not native display frame pacing.

Generate only for intentional changes, inspect images, then compare without updates:

```sh
flutter test --no-pub --update-goldens test/workspace_entrance_visual_test.dart
flutter test --no-pub test/workspace_entrance_visual_test.dart
```

`workspace_entrance_test.dart` separately asserts exact timing, reduced motion, hidden-interaction safety, no replay, disposal, restoration ordering and retained editor/view state. Source fixtures are not installed graphical acceptance or deployment approval.
