import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:homorg/screens/qr_scan_screen.dart';

Widget _app() => const MaterialApp(home: QrScanScreen());

void main() {
  group('QrScanScreen', () {
    testWidgets('shows appbar title and paste button', (tester) async {
      await tester.pumpWidget(_app());
      await tester.pump();

      expect(find.text('Scan QR'), findsOneWidget);
      expect(find.text('Paste URL'), findsOneWidget);
    });

    testWidgets('shows torch and camera switch buttons', (tester) async {
      await tester.pumpWidget(_app());
      await tester.pump();

      expect(find.byIcon(Icons.flash_on), findsOneWidget);
      expect(find.byIcon(Icons.cameraswitch), findsOneWidget);
    });
  });
}
