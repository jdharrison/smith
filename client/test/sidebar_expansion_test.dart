import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/ui/smith_theme.dart';
import 'package:smith_client/ui/smith_workspace.dart';

Finder get navigation => find.byKey(const ValueKey('workspace-navigation'));
Finder button(String tooltip) => find.descendant(
  of: find.byTooltip(tooltip),
  matching: find.byType(IconButton),
);

Future<void> mount(
  WidgetTester tester, {
  Size size = const Size(1280, 720),
  double scale = 1,
}) async {
  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    MaterialApp(
      theme: SmithTheme.dark(),
      builder: (context, child) => MediaQuery(
        data: MediaQuery.of(context)
            .copyWith(textScaler: TextScaler.linear(scale)),
        child: child!,
      ),
      home: const SmithWorkspace(
        chats: [],
        title: 'Fixture chat',
        version: 'test',
        connectionStatus: 'Unknown',
        modelStatus: 'Unknown',
        projects: Column(
          children: [
            Text('Projects page'),
            TextField(key: ValueKey('project-draft')),
          ],
        ),
        agents: TextField(key: ValueKey('agent-draft')),
        devJobs: Text('Dev page'),
        child: Text('Chat page'),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('wide sidebar has top-anchored text and one inline Dev row', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();

    await mount(tester);
    expect(tester.getSize(navigation).width, greaterThanOrEqualTo(184));
    for (final label in ['Chats', 'Dev', 'Settings']) {
      expect(
        find.descendant(of: navigation, matching: find.text(label)),
        findsOneWidget,
      );
    }
    expect(
      tester.getRect(button('Chat')).top,
      closeTo(tester.getRect(navigation).top, 1),
    );
    final root = tester.getRect(button('Dev'));
    final arrow = tester.getRect(button('Dev navigation'));
    expect(root.top, arrow.top);
    expect(root.right, lessThanOrEqualTo(arrow.left));
    expect(arrow.size, const Size(48, 48));
    expect(tester.getSemantics(button('Dev')).getSemanticsData().label, 'Dev');
    semantics.dispose();
    expect(find.text('Projects page'), findsNothing);
    await tester.tap(button('Dev navigation'));
    await tester.pumpAndSettle();
    expect(find.text('Chat page'), findsOneWidget);
    expect(
      find.descendant(of: navigation, matching: find.text('Projects')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: navigation, matching: find.text('Agents')),
      findsNothing,
    );
    expect(tester.getRect(button('Projects')).left, greaterThan(root.left));
    await tester.tap(button('Dev'));
    await tester.pumpAndSettle();
    expect(find.text('Dev page'), findsOneWidget);
    expect(button('Projects'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'collapsing inline Dev links retains page draft and restores focus',
    (tester) async {
      await mount(tester);
      await tester.tap(button('Dev navigation'));
      await tester.pumpAndSettle();
      await tester.tap(button('Projects'));
      await tester.pumpAndSettle();
      final field = find.byKey(const ValueKey('project-draft'));
      await tester.enterText(field, 'Keep this draft');
      final state = tester.state(field);
      final child = tester.widget<IconButton>(button('Projects')).focusNode!;
      child.requestFocus();
      await tester.pump();
      final arrow = tester.widget<IconButton>(button('Dev navigation'));
      arrow.onPressed!();
      await tester.pumpAndSettle();
      expect(button('Projects'), findsNothing);
      expect(tester.state(field), same(state));
      expect(find.text('Keep this draft'), findsOneWidget);
      expect(child.hasFocus, false);
      expect(arrow.focusNode!.hasFocus, true);
      await tester.tap(button('Dev'));
      await tester.pumpAndSettle();
      await tester.tap(button('Dev navigation'));
      await tester.pumpAndSettle();
      await tester.tap(button('Projects'));
      await tester.pumpAndSettle();
      expect(tester.state(field), same(state));
      expect(find.text('Keep this draft'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'compact Dev keeps separate keyboard controls and inline child links',
    (tester) async {
      await mount(tester, size: const Size(400, 400), scale: 2);
      expect(tester.getSize(navigation).width, 104);
      expect(
        find.descendant(of: navigation, matching: find.byType(IconButton)),
        findsNWidgets(4),
      );
      final dev = tester.widget<IconButton>(button('Dev navigation'));
      dev.focusNode!.requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey('dev-chooser')), findsNothing);
      expect(find.text('Chat page'), findsOneWidget);
      expect(button('Agents'), findsNothing);
      final root = tester.getRect(button('Dev'));
      final disclosure = tester.getRect(button('Dev navigation'));
      expect(root.size, const Size(48, 48));
      expect(disclosure.size, const Size(48, 48));
      expect(root.top, disclosure.top);
      expect(root.right, lessThanOrEqualTo(disclosure.left));
      final child = tester.getRect(button('Projects'));
      expect(child.left, greaterThan(root.left));
      expect(child.right, lessThanOrEqualTo(tester.getRect(navigation).right));
      expect(child.bottom, lessThanOrEqualTo(400));
      final devRoot = tester.widget<IconButton>(button('Dev'));
      devRoot.focusNode!.requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.text('Dev page'), findsOneWidget);
      expect(button('Projects'), findsOneWidget);
      final projects = tester.widget<IconButton>(button('Projects'));
      projects.focusNode!.requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(find.text('Projects page'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'reference alignment uses one selected Dev surface and aligned labels',
    (tester) async {
      await mount(tester);
      await tester.tap(button('Dev navigation'));
      await tester.pumpAndSettle();
      await tester.tap(button('Dev'));
      await tester.pumpAndSettle();
      final highlight = find.byKey(const ValueKey('navigation-highlight-Dev'));
      final surface =
          tester.widget<Container>(highlight).decoration! as BoxDecoration;
      expect(surface.color, SmithColors.surfaceSelected);
      final root = tester.getRect(button('Dev'));
      final arrow = tester.getRect(button('Dev navigation'));
      expect(
        tester.getRect(highlight),
        Rect.fromLTRB(root.left, root.top, arrow.right, arrow.bottom),
      );
      expect(root.height, 48);
      final accent = find.descendant(
        of: highlight,
        matching: find.byType(Positioned),
      );
      expect(tester.widget<Positioned>(accent).width, 2);
      expect(
        tester
            .widget<IconButton>(button('Dev navigation'))
            .style!
            .foregroundColor!
            .resolve({}),
        SmithColors.accent,
      );
      double labelLeft(String text) => tester
          .getTopLeft(
            find.descendant(of: navigation, matching: find.text(text)),
          )
          .dx;
      expect(labelLeft('Chats'), labelLeft('Dev'));
      expect(labelLeft('Settings'), labelLeft('Dev'));

      expect(labelLeft('Projects'), greaterThan(labelLeft('Dev')));
      expect(
        tester.getRect(button('Settings')).top -
            tester.getRect(button('Projects')).bottom,
        greaterThanOrEqualTo(8),
      );
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('Projects has no redundant shell title at either width', (
    tester,
  ) async {
    await mount(tester);
    await tester.tap(button('Dev navigation'));
    await tester.pumpAndSettle();
    await tester.tap(button('Projects'));
    await tester.pumpAndSettle();
    final title = find.byWidgetPredicate(
      (widget) => widget is Text && widget.semanticsLabel == 'Projects',
    );
    expect(title, findsNothing);
    expect(find.text('Projects page'), findsOneWidget);
    expect(find.byTooltip('Projects'), findsOneWidget);
    await tester.binding.setSurfaceSize(const Size(400, 400));
    await tester.pumpAndSettle();
    expect(title, findsNothing);
    expect(find.text('Projects page'), findsOneWidget);
    await tester.tap(button('Return to Chat'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('compact-chat-title')), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('wide labels fit enlarged text without overflowing', (
    tester,
  ) async {
    await mount(tester, size: const Size(1800, 800), scale: 2);
    await tester.tap(button('Dev navigation'));
    await tester.pumpAndSettle();
    expect(tester.getSize(navigation).width, greaterThanOrEqualTo(184));
    for (final label in ['Chats', 'Dev', 'Projects', 'Settings']) {
      final text = find.descendant(of: navigation, matching: find.text(label));
      expect(text, findsOneWidget);
      final render = tester.renderObject<RenderParagraph>(text);
      expect(render.didExceedMaxLines, false);
    }
    expect(tester.takeException(), isNull);
  });
}
