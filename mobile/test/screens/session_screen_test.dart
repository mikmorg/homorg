import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:homorg/models/camera_models.dart';
import 'package:homorg/screens/session_screen.dart';
import 'package:homorg/services/api_service.dart';

final _token = 'e' * 64;
final _connection = CameraConnection(baseUrl: 'http://localhost:8080', token: _token);

Widget _app({required http.Client client}) {
  return MaterialApp(
    home: SessionScreen(
      connection: _connection,
      apiServiceFactory: (conn) => ApiService(conn, client: client),
    ),
  );
}

http.Response _statusResponse({
  String sessionId = 'sess-1',
  String? activeContainerId,
  String? activeItemId,
  bool sessionEnded = false,
}) {
  return http.Response(
    jsonEncode({
      'session_id': sessionId,
      'active_container_id': activeContainerId,
      'active_item_id': activeItemId,
      'session_ended': sessionEnded,
    }),
    200,
  );
}

/// Pump enough frames for the initial status fetch to complete.
Future<void> _pumpUntilLoaded(WidgetTester tester) async {
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 100));
}

void main() {
  group('SessionScreen', () {
    testWidgets('shows "Connected" when session is active with an item', (tester) async {
      final client = MockClient((request) async => _statusResponse(
        activeItemId: 'abcdef1234567890abcdef1234567890',
        activeContainerId: 'cont-1',
      ));

      await tester.pumpWidget(_app(client: client));
      await _pumpUntilLoaded(tester);

      expect(find.text('Connected'), findsOneWidget);
      expect(find.text('Ready to take photos'), findsOneWidget);
    });

    testWidgets('shows "Connected" when no active item yet', (tester) async {
      final client = MockClient((request) async => _statusResponse(activeItemId: null));

      await tester.pumpWidget(_app(client: client));
      await _pumpUntilLoaded(tester);

      expect(find.text('Connected'), findsOneWidget);
      expect(find.text('Ready to take photos'), findsOneWidget);
    });

    testWidgets('shows "Session ended" when session is ended', (tester) async {
      final client = MockClient((request) async => _statusResponse(sessionEnded: true));

      await tester.pumpWidget(_app(client: client));
      await _pumpUntilLoaded(tester);

      expect(find.text('Session ended'), findsOneWidget);
    });

    testWidgets('shows "Token expired" on 401', (tester) async {
      final client = MockClient((request) async => http.Response('Unauthorized', 401));

      await tester.pumpWidget(_app(client: client));
      await _pumpUntilLoaded(tester);

      expect(find.text('Token expired'), findsOneWidget);
      expect(find.text('Go back and generate a new camera link'), findsOneWidget);
    });

    testWidgets('shows error state on server error', (tester) async {
      final client = MockClient((request) async => http.Response('Server Error', 500));

      await tester.pumpWidget(_app(client: client));
      await _pumpUntilLoaded(tester);

      expect(find.text('Cannot reach server'), findsOneWidget);
    });

    testWidgets('Take Photo button is enabled when connected (no active item required)', (tester) async {
      final client = MockClient((request) async => _statusResponse(activeItemId: null));

      await tester.pumpWidget(_app(client: client));
      await _pumpUntilLoaded(tester);

      final button = tester.widget<ButtonStyleButton>(
        find.ancestor(of: find.text('Take Photo'), matching: find.bySubtype<ButtonStyleButton>()),
      );
      expect(button.onPressed, isNotNull);
    });

    testWidgets('Take Photo button is disabled when session ended', (tester) async {
      final client = MockClient((request) async => _statusResponse(sessionEnded: true));

      await tester.pumpWidget(_app(client: client));
      await _pumpUntilLoaded(tester);

      final button = tester.widget<ButtonStyleButton>(
        find.ancestor(of: find.text('Take Photo'), matching: find.bySubtype<ButtonStyleButton>()),
      );
      expect(button.onPressed, isNull);
    });

    testWidgets('Take Photo button is disabled when token expired', (tester) async {
      final client = MockClient((request) async => http.Response('Unauthorized', 401));

      await tester.pumpWidget(_app(client: client));
      await _pumpUntilLoaded(tester);

      final button = tester.widget<ButtonStyleButton>(
        find.ancestor(of: find.text('Take Photo'), matching: find.bySubtype<ButtonStyleButton>()),
      );
      expect(button.onPressed, isNull);
    });

    testWidgets('shows Refresh button in AppBar when not expired', (tester) async {
      final client = MockClient((request) async => _statusResponse(activeItemId: null));

      await tester.pumpWidget(_app(client: client));
      await _pumpUntilLoaded(tester);

      expect(find.byIcon(Icons.refresh), findsOneWidget);
    });

    testWidgets('hides Refresh button in AppBar when token expired', (tester) async {
      final client = MockClient((request) async => http.Response('Unauthorized', 401));

      await tester.pumpWidget(_app(client: client));
      await _pumpUntilLoaded(tester);

      expect(find.byIcon(Icons.refresh), findsNothing);
    });

    testWidgets('displays connection base URL at bottom', (tester) async {
      final client = MockClient((request) async => _statusResponse(activeItemId: null));

      await tester.pumpWidget(_app(client: client));
      await _pumpUntilLoaded(tester);

      expect(find.text('http://localhost:8080'), findsOneWidget);
    });

    testWidgets('AppBar title is Stocker Session', (tester) async {
      final client = MockClient((request) async => _statusResponse(activeItemId: null));

      await tester.pumpWidget(_app(client: client));
      await _pumpUntilLoaded(tester);

      expect(find.text('Stocker Session'), findsOneWidget);
    });
  });
}
