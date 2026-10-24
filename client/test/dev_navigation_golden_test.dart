import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/ui/smith_theme.dart';
import 'package:smith_client/ui/smith_workspace.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    await (FontLoader(
      'MaterialIcons',
    )..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'))).load();
    await (FontLoader(SmithTheme.defaultFontFamily)..addFont(
          rootBundle.load(
            'assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf',
          ),
        ))
        .load();
  });
  for (final (name, size, scale, chooser) in [
    ('wide-expanded', const Size(1200, 700), 1.0, false),
    ('compact-root-text2', const Size(400, 400), 2.0, false),
    ('compact-chooser-text2', const Size(400, 400), 2.0, true),
  ]) {
    testWidgets('Dev bundled-font fixture $name', (tester) async {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(
        RepaintBoundary(
          key: const ValueKey('dev-capture'),
          child: MaterialApp(
            debugShowCheckedModeBanner: false,
            theme: SmithTheme.dark(),
            builder: (context, child) => MediaQuery(
              data: MediaQuery.of(context)
                  .copyWith(textScaler: TextScaler.linear(scale)),
              child: child!,
            ),
            home: const SmithWorkspace(
              page: 'dev',
              chats: [],
              title: 'Retained chat',
              version: 'test',
              connectionStatus: 'SYSTEM ONLINE',
              modelStatus: 'test',
              connectionOnline: true,
              child: Text('Hidden chat'),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      if (chooser || size.width > 960) {
        await tester.tap(find.byTooltip('Dev navigation'));
        await tester.pumpAndSettle();
      }
      FocusManager.instance.primaryFocus?.unfocus();
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await expectLater(
        find.byKey(const ValueKey('dev-capture')),
        matchesGoldenFile('goldens/dev-navigation/$name.png'),
      );
      await tester.pumpWidget(const SizedBox());
    });
  }
}
