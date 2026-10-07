import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/home/settings_tab.dart';

void main() {
  group('ServicesSnapshot compatibility', () {
    test('legacy response supplies unknown supervisor observations', () {
      final snapshot = ServicesSnapshot.fromJson(const <String, dynamic>{
        'autostart': true,
        'os_registered': true,
        'services': <dynamic>[],
      });

      expect(snapshot.autostart, isTrue);
      expect(snapshot.osRegistered, isTrue);
      expect(snapshot.supervisor.registered, isTrue);
      expect(snapshot.supervisor.loadState, 'unknown');
      expect(snapshot.supervisor.enabledState, 'unknown');
      expect(snapshot.supervisor.enabledAtLogin, isNull);
      expect(snapshot.supervisor.activeState, 'unknown');
      expect(snapshot.supervisor.subState, 'unknown');
      expect(snapshot.supervisor.linger, isNull);
      expect(snapshot.supervisor.bridgeIdentity, 'unknown');
      expect(snapshot.supervisor.bridgeHealthy, isFalse);
      expect(snapshot.supervisor.detail, isNull);
    });

    test('additive supervisor response decodes every observation', () {
      final snapshot = ServicesSnapshot.fromJson(const <String, dynamic>{
        'autostart': false,
        'os_registered': true,
        'supervisor': <String, dynamic>{
          'registered': true,
          'load_state': 'loaded',
          'enabled_state': 'enabled',
          'enabled_at_login': true,
          'active_state': 'active',
          'sub_state': 'running',
          'linger': false,
          'bridge_identity': 'smith',
          'bridge_healthy': true,
          'detail': 'observed from user manager',
        },
        'services': <dynamic>[],
      });

      expect(snapshot.autostart, isFalse);
      expect(snapshot.osRegistered, isTrue);
      expect(snapshot.supervisor.registered, isTrue);
      expect(snapshot.supervisor.loadState, 'loaded');
      expect(snapshot.supervisor.enabledState, 'enabled');
      expect(snapshot.supervisor.enabledAtLogin, isTrue);
      expect(snapshot.supervisor.activeState, 'active');
      expect(snapshot.supervisor.subState, 'running');
      expect(snapshot.supervisor.linger, isFalse);
      expect(snapshot.supervisor.bridgeIdentity, 'smith');
      expect(snapshot.supervisor.bridgeHealthy, isTrue);
      expect(snapshot.supervisor.detail, 'observed from user manager');
    });
  });

  testWidgets(
    'supervisor settings label configured and observed state exactly',
    (tester) async {
      final snapshot = ServicesSnapshot.fromJson(const <String, dynamic>{
        'autostart': true,
        'os_registered': true,
        'supervisor': <String, dynamic>{
          'registered': true,
          'load_state': 'loaded',
          'enabled_state': 'disabled',
          'enabled_at_login': false,
          'active_state': 'inactive',
          'sub_state': 'dead',
          'linger': null,
          'bridge_identity': 'smith',
          'bridge_healthy': true,
        },
        'services': <dynamic>[],
      });

      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 480,
              child: SupervisorStatusSummary(
                snapshot: snapshot,
                busy: false,
                onAutostartChanged: (_) {},
              ),
            ),
          ),
        ),
      );

      expect(find.text('Configured autostart preference'), findsOneWidget);
      expect(
        find.text(
          'Configured preference: on. Observed service state may differ.',
        ),
        findsOneWidget,
      );
      expect(find.text('unit registration'), findsOneWidget);
      expect(find.text('unit load state'), findsOneWidget);
      expect(find.text('runtime active state'), findsOneWidget);
      expect(find.text('runtime substate'), findsOneWidget);
      expect(find.text('enabled at login'), findsOneWidget);
      expect(find.text('unit enablement state'), findsOneWidget);
      expect(find.text('linger'), findsOneWidget);
      expect(find.text('bridge identity'), findsOneWidget);
      expect(find.text('bridge health'), findsOneWidget);
      expect(find.text('loaded'), findsOneWidget);
      expect(find.text('disabled'), findsOneWidget);
      expect(find.text('inactive'), findsOneWidget);
      expect(find.text('dead'), findsOneWidget);
      expect(find.text('unknown'), findsOneWidget);
      expect(find.text('enable to install'), findsNothing);
    },
  );
}
