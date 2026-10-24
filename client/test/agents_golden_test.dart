import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/agents_controller.dart';
import 'package:smith_client/ui/agents_view.dart';
import 'package:smith_client/ui/smith_theme.dart';

import 'agents_view_test.dart'
    show AgentsUiApi, agentTap, agentKey, agentAction;

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
  for (final (name, size, scale, editor, dpr) in [
    ('wide', const Size(1536, 1024), 1.0, true, 1.0),
    ('large-anchor', const Size(2400, 1200), 1.0, true, 1.0),
    ('compact-list', const Size(400, 400), 1.0, false, 1.0),
    ('compact-editor', const Size(400, 400), 1.0, true, 1.0),
    ('compact-list-text2', const Size(400, 400), 2.0, false, 1.0),
    ('compact-editor-text2', const Size(400, 400), 2.0, true, 1.0),
    ('compact-editor-dpi2', const Size(800, 800), 1.0, true, 2.0),
    ('new-editor', const Size(1536, 1024), 1.0, true, 1.0),
    ('custom-editor', const Size(1536, 1024), 1.0, true, 1.0),
  ]) {
    testWidgets('Agents bundled-font fixture $name', (tester) async {
      tester.view.physicalSize = size;
      tester.view.devicePixelRatio = dpr;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final controller = AgentsController(
        AgentsUiApi(custom: name == 'custom-editor'),
      );
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        MaterialApp(
          debugShowCheckedModeBanner: false,
          theme: SmithTheme.dark(),
          builder: (context, child) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(scale)),
            child: child!,
          ),
          home: RepaintBoundary(
            key: const ValueKey('agents-capture'),
            child: Scaffold(body: AgentsView(controller: controller)),
          ),
        ),
      );
      await tester.pumpAndSettle();
      if (name == 'new-editor') {
        await agentTap(tester, agentAction('NEW AGENT'));
      } else if (editor) {
        await agentTap(
          tester,
          agentKey(
            name == 'custom-editor'
                ? 'agent-card-${AgentsUiApi.customId}'
                : 'agent-card-developer',
          ),
        );
      }
      expect(find.text('Search agents'), findsNothing);
      expect(find.text('Refresh agents'), findsNothing);
      FocusManager.instance.primaryFocus?.unfocus();
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await expectLater(
        find.byKey(const ValueKey('agents-capture')),
        matchesGoldenFile('goldens/agents/$name.png'),
      );
      await tester.pumpWidget(const SizedBox());
    });
  }
}
