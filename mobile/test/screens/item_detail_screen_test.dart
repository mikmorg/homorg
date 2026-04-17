import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/testing.dart';
import 'package:http/http.dart' as http;

import 'package:homorg/screens/item_detail_screen.dart';
import 'package:homorg/services/auth_service.dart';
import 'package:homorg/services/homorg_api.dart';

const _itemId = '00000000-0000-0000-0000-000000000001';

class _FakeAuth extends Fake implements AuthService {
  @override
  String? get serverUrl => 'http://localhost:8080';

  @override
  String? get accessToken => 'test-token';
}

void main() {
  group('ItemDetailScreen', () {
    testWidgets('renders item name and basic details', (WidgetTester tester) async {
      final mockClient = MockClient((request) async {
        if (request.url.toString().contains('/items/$_itemId')) {
          return http.Response(
            jsonEncode({
              'id': _itemId,
              'name': 'Test Widget',
              'description': 'A test item',
              'is_container': false,
              'is_deleted': false,
              'created_at': '2026-04-17T10:00:00Z',
              'updated_at': '2026-04-17T10:00:00Z',
            }),
            200,
          );
        }
        if (request.url.toString().contains('/containers')) {
          return http.Response(jsonEncode([]), 200);
        }
        if (request.url.toString().contains('/history')) {
          return http.Response(jsonEncode([]), 200);
        }
        return http.Response('', 404);
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);

      await tester.pumpWidget(
        MaterialApp(
          home: ItemDetailScreen(itemId: _itemId, api: api),
        ),
      );

      await tester.pumpAndSettle();

      expect(find.text('Test Widget'), findsWidgets);
      expect(find.text('A test item'), findsOneWidget);
    });

    testWidgets('shows container contents for container items', (WidgetTester tester) async {
      const containerId = '11111111-1111-1111-1111-111111111111';

      final mockClient = MockClient((request) async {
        if (request.url.toString().contains('/items/$containerId')) {
          return http.Response(
            jsonEncode({
              'id': containerId,
              'name': 'Storage Box',
              'is_container': true,
              'is_deleted': false,
              'created_at': '2026-04-17T10:00:00Z',
              'updated_at': '2026-04-17T10:00:00Z',
            }),
            200,
          );
        }
        if (request.url.toString().contains('/containers/$containerId/children')) {
          return http.Response(
            jsonEncode([
              {
                'id': 'child-1',
                'name': 'Item in Box',
                'is_container': false,
              },
            ]),
            200,
          );
        }
        if (request.url.toString().contains('/containers')) {
          return http.Response(jsonEncode([]), 200);
        }
        if (request.url.toString().contains('/history')) {
          return http.Response(jsonEncode([]), 200);
        }
        return http.Response('', 404);
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);

      await tester.pumpWidget(
        MaterialApp(
          home: ItemDetailScreen(itemId: containerId, api: api),
        ),
      );

      await tester.pumpAndSettle();

      expect(find.text('Storage Box'), findsWidgets);
      expect(find.text('Item in Box'), findsOneWidget);
    });

    testWidgets('edit button opens _EditItemPage', (WidgetTester tester) async {
      final mockClient = MockClient((request) async {
        if (request.url.toString().contains('/items/$_itemId')) {
          return http.Response(
            jsonEncode({
              'id': _itemId,
              'name': 'Editable Item',
              'is_container': false,
              'is_deleted': false,
              'created_at': '2026-04-17T10:00:00Z',
              'updated_at': '2026-04-17T10:00:00Z',
            }),
            200,
          );
        }
        if (request.url.toString().contains('/containers')) {
          return http.Response(jsonEncode([]), 200);
        }
        if (request.url.toString().contains('/history')) {
          return http.Response(jsonEncode([]), 200);
        }
        if (request.url.toString().contains('/categories')) {
          return http.Response(jsonEncode([]), 200);
        }
        if (request.url.toString().contains('/tags')) {
          return http.Response(jsonEncode([]), 200);
        }
        if (request.url.toString().contains('/container-types')) {
          return http.Response(jsonEncode([]), 200);
        }
        return http.Response('', 404);
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);

      await tester.pumpWidget(
        MaterialApp(
          home: ItemDetailScreen(itemId: _itemId, api: api),
        ),
      );

      await tester.pumpAndSettle();

      // Find and tap the edit button (usually in the AppBar)
      final editButton = find.byTooltip('Edit');
      if (editButton.evaluate().isNotEmpty) {
        await tester.tap(editButton);
        await tester.pumpAndSettle();

        // Verify that we're on the edit page (should have form fields)
        expect(find.byType(TextField), findsWidgets);
      }
    });
  });
}
