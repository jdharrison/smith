import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/chat_panel.dart';
import 'package:smith_client/main.dart';
import 'package:smith_client/ui/settings_view.dart';

import 'app_fakes.dart';
import 'widget_test.dart' show FakeControlApi, profile;

Widget settings({
  String connection = 'Last observed connection unavailable',
  String model = 'Model availability not currently verified.',
  String version = 'Development build',
  double scale = 1,
}) => MaterialApp(
  home: MediaQuery(
    data: MediaQueryData(textScaler: TextScaler.linear(scale)),
    child: Scaffold(
      body: SettingsView(
        connectionStatus: connection,
        modelStatus: model,
        version: version,
      ),
    ),
  ),
);

void main() {
  testWidgets('settings exposes only supplied read-only facts', (tester) async {
    final semantics = tester.ensureSemantics();

    await tester.binding.setSurfaceSize(const Size(1280, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(settings());

    expect(find.text('Last observed connection unavailable'), findsOneWidget);
    expect(
      find.text('Model availability not currently verified.'),
      findsOneWidget,
    );
    expect(find.text('Development build'), findsOneWidget);
    expect(find.byType(SelectableText), findsNWidgets(3));
    expect(find.byType(TextField), findsNothing);
    expect(find.byType(Switch), findsNothing);
    expect(find.byType(Checkbox), findsNothing);
    expect(find.byType(Slider), findsNothing);
    expect(find.byType(IconButton), findsNothing);
    expect(find.byType(TextButton), findsNothing);
    expect(
      tester
          .getSemantics(find.text('Connection & About'))
          .flagsCollection
          .isHeader,
      isTrue,
    );

    await tester.pumpWidget(
      settings(
        connection: 'SYSTEM ONLINE',
        model: 'Selected model: local:exact. Inference not verified.',
        version: 'v1.2.3+4',
      ),
    );
    expect(find.text('Last observed connection unavailable'), findsNothing);
    expect(find.text('SYSTEM ONLINE'), findsOneWidget);
    expect(find.text('v1.2.3+4'), findsOneWidget);
    expect(find.textContaining('Inference not verified.'), findsOneWidget);
    expect(
      find.textContaining('not a guarantee of model readiness'),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
    semantics.dispose();
  });

  for (final size in [const Size(360, 800), const Size(400, 400)]) {
    testWidgets('settings disclosures reachable at $size with enlarged text', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(size);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(settings(scale: 2));
      expect(tester.takeException(), isNull);
      for (final text in [
        'Version',
        'Conversation history and model context are bounded',
        'Closing this window detaches the client.',
        'Model setup and service maintenance belong in the separate installer',
      ]) {
        final target = find.textContaining(text);
        await tester.scrollUntilVisible(
          target,
          100,
          scrollable: find.byType(Scrollable).first,
        );
        await tester.ensureVisible(target);
        expect(tester.getRect(target).overlaps(Offset.zero & size), isTrue);
        // Enlarged disclosures may be taller than the viewport; their end
        // must also be reachable instead of requiring a visible midpoint.
        await Scrollable.ensureVisible(tester.element(target), alignment: 1);
        await tester.pumpAndSettle();
        expect(tester.getRect(target).bottom, lessThanOrEqualTo(size.height));
        expect(tester.takeException(), isNull);
      }
    });
  }

  testWidgets(
    'gear below Chat opens settings without calls or chat state loss',
    (tester) async {
      final api = FakeAppApi();
      final control = FakeControlApi(profile());
      for (var i = 0; i < 32; i++) {
        api.history.add({
          'role': i.isEven ? 'user' : 'assistant',
          'content': 'Saved message $i',
        });
      }
      await tester.pumpWidget(SmithApp(controlApi: control, appApi: api));
      await tester.pumpAndSettle();

      final chatButton = find.byTooltip('Open chat history');
      final settingsButton = find.byTooltip('Settings');
      final chatRect = tester.getRect(chatButton);
      final settingsRect = tester.getRect(settingsButton);
      expect(settingsRect.top, greaterThanOrEqualTo(chatRect.bottom));
      expect(settingsRect.center.dx, chatRect.center.dx);

      final panelState = tester.state(find.byType(ChatPanel));
      final history = find.byKey(const ValueKey('conversation-history'));
      final scroll = tester.widget<SingleChildScrollView>(history).controller!;
      await tester.drag(history, const Offset(0, 300));
      await tester.pumpAndSettle();
      final readingOffset = scroll.offset;
      expect(readingOffset, lessThan(scroll.position.maxScrollExtent));
      await tester.enterText(find.byType(TextField), 'Retain my unsent draft');
      final appCalls = List<String>.of(api.calls);
      final controlCalls = List<String>.of(control.calls);

      await tester.tap(settingsButton);
      await tester.pumpAndSettle();
      expect(find.byType(SettingsView), findsOneWidget);
      expect(find.byType(TextField), findsNothing);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(api.calls, appCalls);
      expect(control.calls, controlCalls);

      await tester.tap(find.byTooltip('Return to Chat'));
      await tester.pumpAndSettle();
      expect(find.byType(SettingsView), findsNothing);
      expect(tester.state(find.byType(ChatPanel)), same(panelState));
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'Retain my unsent draft',
      );
      expect(scroll.offset, readingOffset);
      expect(api.calls, appCalls);
      expect(control.calls, controlCalls);
      expect(api.disposed, isFalse);
      expect(control.disposed, isFalse);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
    },
  );
}
