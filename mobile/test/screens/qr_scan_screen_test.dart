import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:homorg_camera/screens/qr_scan_screen.dart';

Widget _app() => const MaterialApp(home: QrScanScreen());

void _mockClipboard(WidgetTester tester, String? text) {
  TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(SystemChannels.platform, (call) async {
    if (call.method == 'Clipboard.getData') {
      if (text == null) return null;
      return <String, dynamic>{'text': text};
    }
    return null;
  });
}

void main() {
  group('QrScanScreen', () {
    testWidgets('shows instructions and title', (tester) async {
      _mockClipboard(tester, null);
      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();

      expect(find.text('Paste URL'), findsOneWidget);
      expect(find.textContaining('Copy the upload URL'), findsOneWidget);
    });

    testWidgets('shows "Check clipboard" when no camera URL in clipboard', (tester) async {
      _mockClipboard(tester, 'just some random text');
      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();

      expect(find.text('Check clipboard'), findsOneWidget);
      expect(find.text('Use this URL'), findsNothing);
    });

    testWidgets('shows "Check clipboard" when clipboard is empty', (tester) async {
      _mockClipboard(tester, null);
      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();

      expect(find.text('Check clipboard'), findsOneWidget);
    });

    testWidgets('shows detected URL and "Use this URL" button when camera URL in clipboard', (tester) async {
      final token = 'f' * 64;
      final url = 'http://host:8080/api/v1/stocker/camera/$token/upload';
      _mockClipboard(tester, url);

      await tester.pumpWidget(_app());
      await tester.pumpAndSettle();

      expect(find.text('URL detected in clipboard:'), findsOneWidget);
      expect(find.text(url), findsOneWidget);
      expect(find.text('Use this URL'), findsOneWidget);
    });

    testWidgets('"Use this URL" pops with the URL', (tester) async {
      final token = 'a1b2c3d4' * 8; // 64 chars
      final url = 'http://host:8080/api/v1/stocker/camera/$token/upload';
      String? returnedUrl;

      _mockClipboard(tester, url);

      await tester.pumpWidget(MaterialApp(
        home: Builder(
          builder: (context) => Scaffold(
            body: ElevatedButton(
              onPressed: () async {
                returnedUrl = await Navigator.push<String>(
                  context,
                  MaterialPageRoute(builder: (_) => const QrScanScreen()),
                );
              },
              child: const Text('Go'),
            ),
          ),
        ),
      ));

      await tester.tap(find.text('Go'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Use this URL'));
      await tester.pumpAndSettle();

      expect(returnedUrl, url);
    });
  });
}
