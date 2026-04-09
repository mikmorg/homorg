import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:homorg_camera/screens/connect_screen.dart';
import 'package:shared_preferences/shared_preferences.dart';

Widget _app() => const MaterialApp(home: ConnectScreen());

/// Find the Connect button — it's a FilledButton.icon, so we match via ancestor.
Finder _connectButton() => find.ancestor(
      of: find.text('Connect'),
      matching: find.bySubtype<ButtonStyleButton>(),
    );

/// Helper to tap the connect button by invoking its onPressed directly.
/// This avoids scrolling/visibility issues in the test harness.
Future<void> _tapConnect(WidgetTester tester) async {
  final button = tester.widget<ButtonStyleButton>(_connectButton());
  expect(button.onPressed, isNotNull, reason: 'Connect button should be enabled');
  button.onPressed!();
  await tester.pump();
}

void main() {
  group('ConnectScreen', () {
    setUp(() {
      SharedPreferences.setMockInitialValues({});
    });

    testWidgets('renders title and subtitle', (tester) async {
      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();
      expect(find.text('Homorg Camera'), findsOneWidget);
      expect(find.text('Attach photos to stocker sessions'), findsOneWidget);
    });

    testWidgets('renders URL text field with label', (tester) async {
      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();
      expect(find.text('Upload URL'), findsOneWidget);
      expect(find.byType(TextField), findsOneWidget);
    });

    testWidgets('Connect button is disabled when URL is empty', (tester) async {
      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();

      final button = tester.widget<ButtonStyleButton>(_connectButton());
      expect(button.onPressed, isNull);
    });

    testWidgets('Connect button is enabled when URL has text', (tester) async {
      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'some text');
      await tester.pumpAndSettle();

      final button = tester.widget<ButtonStyleButton>(_connectButton());
      expect(button.onPressed, isNotNull);
    });

    testWidgets('shows validation error for invalid URL', (tester) async {
      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'not a valid url');
      await tester.pumpAndSettle();
      await _tapConnect(tester);
      expect(
        find.text('Paste the full upload URL from the stocker page camera panel'),
        findsOneWidget,
      );
    });

    testWidgets('clears validation error when typing', (tester) async {
      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'bad');
      await tester.pumpAndSettle();
      await _tapConnect(tester);
      expect(
        find.text('Paste the full upload URL from the stocker page camera panel'),
        findsOneWidget,
      );
      await tester.enterText(find.byType(TextField), 'bad2');
      await tester.pump();
      expect(
        find.text('Paste the full upload URL from the stocker page camera panel'),
        findsNothing,
      );
    });

    testWidgets('renders QR scan button', (tester) async {
      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();
      expect(find.byIcon(Icons.qr_code_scanner), findsOneWidget);
    });

    testWidgets('does not show Recent section when no saved URLs', (tester) async {
      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();
      expect(find.text('Recent'), findsNothing);
    });

    testWidgets('shows Recent section with saved URLs', (tester) async {
      final token = 'b' * 64;
      final savedUrl = 'http://host:8080/api/v1/stocker/camera/$token/upload';
      SharedPreferences.setMockInitialValues({
        'recent_urls': '["$savedUrl"]',
      });

      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();

      expect(find.text('Recent'), findsOneWidget);
      expect(find.text(savedUrl), findsOneWidget);
      expect(find.byIcon(Icons.history), findsOneWidget);
    });

    testWidgets('navigates to SessionScreen on valid URL connect', (tester) async {
      final token = 'c' * 64;
      final url = 'http://host:8080/api/v1/stocker/camera/$token/upload';

      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), url);
      await tester.pumpAndSettle();
      await _tapConnect(tester);
      await tester.pump(const Duration(milliseconds: 100));

      expect(find.text('Stocker Session'), findsOneWidget);
    });

    testWidgets('tapping a recent URL navigates to SessionScreen', (tester) async {
      final token = 'd' * 64;
      final savedUrl = 'http://host:8080/api/v1/stocker/camera/$token/upload';
      SharedPreferences.setMockInitialValues({
        'recent_urls': '["$savedUrl"]',
      });

      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();

      await tester.ensureVisible(find.text(savedUrl));
      await tester.tap(find.text(savedUrl));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));

      expect(find.text('Stocker Session'), findsOneWidget);
    });
  });
}
