import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/ui/build_info.dart';
import 'package:smith_client/ui/smith_icon.dart';
import 'package:smith_client/ui/smith_theme.dart';

class _VersionBundle extends CachingAssetBundle {
  _VersionBundle(this.source, {this.pubspec});
  final String? source;
  final String? pubspec;

  @override
  Future<ByteData> load(String key) async {
    expect(key, anyOf('version.json', 'pubspec.yaml'));
    final content = key == 'version.json' ? source : pubspec;
    if (content == null) throw FlutterError('Missing test metadata');
    return ByteData.sublistView(Uint8List.fromList(utf8.encode(content)));
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'foundation bundles and loads all six real font weights and notices',
    () async {
      final manifest =
          jsonDecode(await rootBundle.loadString('FontManifest.json')) as List;
      for (final family in [
        SmithTheme.defaultFontFamily,
        SmithTheme.alternativeFontFamily,
      ]) {
        final entry = manifest.cast<Map<String, dynamic>>().singleWhere(
          (e) => e['family'] == family,
        );
        final fonts = (entry['fonts'] as List).cast<Map<String, dynamic>>();
        expect(fonts.map((f) => f['weight']), [400, 500, 600]);
        final loader = FontLoader(family);
        for (final font in fonts) {
          final bytes = await rootBundle.load(font['asset'] as String);
          expect(bytes.getUint32(0), 0x00010000);
          expect(bytes.lengthInBytes, greaterThan(50000));
          loader.addFont(Future.value(bytes));
        }
        await loader.load();
        final theme = SmithTheme.dark(fontFamily: family);
        expect(theme.textTheme.bodyMedium?.fontFamily, family);
      }
      for (final directory in ['jetbrains-mono', 'ibm-plex-mono']) {
        expect(
          await rootBundle.loadString('assets/fonts/$directory/OFL.txt'),
          contains('SIL OPEN FONT LICENSE'),
        );
      }
      expect(
        await rootBundle.loadString('assets/README.md'),
        contains('Publication remains blocked'),
      );
    },
  );

  test(
    'foundation text and meaningful focus colors meet contrast on surfaces',
    () {
      double contrast(Color a, Color b) {
        final luminances = [a.computeLuminance(), b.computeLuminance()]..sort();
        return (luminances.last + 0.05) / (luminances.first + 0.05);
      }

      for (final surface in [
        SmithColors.background,
        SmithColors.surface,
        SmithColors.surfaceRaised,
        SmithColors.surfaceSelected,
      ]) {
        for (final text in [
          SmithColors.textPrimary,
          SmithColors.textSecondary,
        ]) {
          expect(contrast(text, surface), greaterThanOrEqualTo(4.5));
        }
        expect(
          contrast(SmithColors.accentBright, surface),
          greaterThanOrEqualTo(3),
        );
      }
      // Muted metadata is used on the main/raised surfaces, not selected rows.
      for (final surface in [
        SmithColors.background,
        SmithColors.surface,
        SmithColors.surfaceRaised,
      ]) {
        expect(
          contrast(SmithColors.textMuted, surface),
          greaterThanOrEqualTo(4.5),
        );
      }
      final theme = SmithTheme.dark();
      expect(theme.focusColor, SmithColors.surfaceSelected);
      expect(
        theme.textSelectionTheme.selectionColor,
        SmithColors.surfaceSelected,
      );
      expect(
        theme.focusColor.computeLuminance(),
        greaterThan(SmithColors.surfaceRaised.computeLuminance()),
      );
      expect(theme.textSelectionTheme.cursorColor, SmithColors.accent);
      expect(theme.extension<SmithTokens>()?.hitTarget, 48);
      final style = theme.iconButtonTheme.style!;
      expect(style.minimumSize!.resolve({}), const Size(48, 48));
      expect(
        style.side!.resolve({WidgetState.focused})?.color,
        SmithColors.accentBright,
      );
      expect(
        style.foregroundColor!.resolve({WidgetState.disabled}),
        SmithColors.textMuted,
      );
      expect(
        style.overlayColor!.resolve({WidgetState.hovered}),
        isNot(Colors.transparent),
      );
    },
  );

  testWidgets(
    'foundation every approved SVG renders locally with explicit tint',
    (tester) async {
      for (final name in SmithIcon.names) {
        final source = await rootBundle.loadString('assets/icons/$name.svg');
        expect(source, isNot(contains('currentColor')));
        expect(source, isNot(contains('href=')));
        await tester.pumpWidget(
          MaterialApp(
            theme: SmithTheme.dark(),
            home: Center(
              child: SmithIcon(
                name,
                color: SmithColors.accent,
                semanticLabel: name,
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        final picture = tester.widget<SvgPicture>(find.byType(SvgPicture));
        expect(
          picture.colorFilter,
          const ColorFilter.mode(SmithColors.accent, BlendMode.srcIn),
        );
        expect(find.bySemanticsLabel(name), findsOneWidget);
        expect(tester.takeException(), isNull);
      }
    },
  );

  testWidgets('foundation SVG tint reaches rendered pixels', (tester) async {
    final key = GlobalKey();
    await tester.pumpWidget(
      MaterialApp(
        home: Center(
          child: RepaintBoundary(
            key: key,
            child: const SmithIcon('check', size: 48, color: Color(0xFFFF0000)),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    final boundary =
        key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
    final image = (await tester.runAsync(() => boundary.toImage()))!;
    final bytes = (await tester.runAsync(
      () => image.toByteData(format: ui.ImageByteFormat.rawRgba),
    ))!;
    var painted = 0;
    for (var i = 0; i < bytes.lengthInBytes; i += 4) {
      if (bytes.getUint8(i + 3) > 128) {
        painted++;
        // rawRgba stores premultiplied channels at antialiased edges.
        expect(bytes.getUint8(i), bytes.getUint8(i + 3));
        expect(bytes.getUint8(i + 1), 0);
        expect(bytes.getUint8(i + 2), 0);
      }
    }
    image.dispose();
    expect(painted, greaterThan(20));
  });

  test(
    'foundation version uses generated pubspec metadata, not a literal',
    () async {
      // Flutter test bundles use the bundled declared version.
      final pubspec = File('pubspec.yaml').readAsStringSync();
      final version = RegExp(
        r'^version: (.+)$',
        multiLine: true,
      ).firstMatch(pubspec)!.group(1)!;
      expect((await BuildInfo.load()).buildVersion, 'v$version');
      final parts = version.split('+');
      final info = await BuildInfo.load(
        bundle: _VersionBundle(
          jsonEncode({
            'version': parts.first,
            'build_number': parts.length == 2 ? parts.last : null,
          }),
        ),
      );
      expect(info.buildVersion, 'v$version');
      expect(
        (await BuildInfo.load(
          bundle: _VersionBundle(
            '{"version":"3.2.1-rc.2","build_number":"17"}',
          ),
        )).buildVersion,
        'v3.2.1-rc.2+17',
      );
      for (final metadata in [null, '{', '{"version":"invalid"}']) {
        expect(
          (await BuildInfo.load(
            bundle: _VersionBundle(metadata, pubspec: pubspec),
          )).buildVersion,
          'v$version',
        );
      }
      expect(
        (await BuildInfo.load(
          bundle: _VersionBundle(
            '{"version":"2.3.4","build_number":"9"}',
            pubspec: pubspec,
          ),
        )).buildVersion,
        'v2.3.4+9',
      );
      for (final declared in ['', 'version: invalid', 'name: smith_client']) {
        expect(
          (await BuildInfo.load(
            bundle: _VersionBundle(null, pubspec: declared),
          )).buildVersion,
          'Version unavailable',
        );
      }
      for (final source in [
        null,
        '{',
        '[]',
        '{"version":""}',
        '{"version":"1.2.3","build_number":7}',
      ]) {
        expect(
          (await BuildInfo.load(bundle: _VersionBundle(source))).buildVersion,
          'Version unavailable',
        );
      }
    },
  );
}
