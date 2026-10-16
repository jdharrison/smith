# Boot presentation fixtures

Synthetic Flutter captures of the production `SmithApp` composition with fake service/app APIs, not installed-desktop or live-service evidence. No service is started and no model/provider request is made.

`boot_visual_test.dart` captures a 1280×720 intro at 250ms, fully visible hold at 500ms, successful outro at its 250ms midpoint, and a scrolled 400×400 repair state at 2× text. The repair action remains reachable without overflow; scrolling is expected at this short/enlarged size. The bundled JetBrains Mono Regular font is explicitly loaded for glyph captures; this does not establish production font-weight selection fidelity.

Generate only after intentional visual changes, inspect all images, then compare without updating:

```sh
flutter test --no-pub --update-goldens test/boot_visual_test.dart
flutter test --no-pub test/boot_visual_test.dart
```

Exact phase/readiness/motion/lifecycle behavior, including a delayed first opaque frame, is independently asserted in `boot_test.dart`. PNGs are visual regression fixtures, not proof of motion cadence or release/deployment approval.
