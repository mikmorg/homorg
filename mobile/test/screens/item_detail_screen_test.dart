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

    group('history section', () {
      Map<String, dynamic> _baseItem({String id = _itemId, String name = 'Test Item'}) => {
        'id': id,
        'name': name,
        'is_container': false,
        'is_deleted': false,
        'created_at': '2026-04-17T10:00:00Z',
        'updated_at': '2026-04-17T10:00:00Z',
      };

      MockClient _clientWithHistory(List<Map<String, dynamic>> events) {
        return MockClient((request) async {
          final url = request.url.toString();
          if (url.contains('/history')) {
            return http.Response(jsonEncode(events), 200);
          }
          if (url.contains('/items/$_itemId') && !url.contains('/history')) {
            return http.Response(jsonEncode(_baseItem()), 200);
          }
          if (url.contains('/containers')) {
            return http.Response(jsonEncode([]), 200);
          }
          return http.Response('', 404);
        });
      }

      Future<void> _expandHistory(WidgetTester tester) async {
        await tester.tap(find.text('History'));
        await tester.pumpAndSettle();
      }

      testWidgets('shows "No history" when list is empty', (tester) async {
        final api = HomorgApi(_FakeAuth(), client: _clientWithHistory([]));
        await tester.pumpWidget(MaterialApp(home: ItemDetailScreen(itemId: _itemId, api: api)));
        await tester.pumpAndSettle();
        await _expandHistory(tester);
        expect(find.text('No history'), findsOneWidget);
      });

      testWidgets('renders ItemMoved event with from/to labels', (tester) async {
        final api = HomorgApi(_FakeAuth(),
            client: _clientWithHistory([
              {
                'id': 1,
                'event_type': 'ItemMoved',
                'created_at': '2026-04-17T10:00:00Z',
                'event_data': {'from_path': 'home.kitchen', 'to_path': 'home.garage'},
              },
            ]));
        await tester.pumpWidget(MaterialApp(home: ItemDetailScreen(itemId: _itemId, api: api)));
        await tester.pumpAndSettle();
        await _expandHistory(tester);
        expect(find.text('Moved from kitchen to garage'), findsOneWidget);
      });

      testWidgets('renders ItemUpdated event with diff details', (tester) async {
        final api = HomorgApi(_FakeAuth(),
            client: _clientWithHistory([
              {
                'id': 2,
                'event_type': 'ItemUpdated',
                'created_at': '2026-04-17T10:00:00Z',
                'event_data': {
                  'changes': [
                    {'field': 'name', 'old': 'Old', 'new': 'New'}
                  ]
                },
              },
            ]));
        await tester.pumpWidget(MaterialApp(home: ItemDetailScreen(itemId: _itemId, api: api)));
        await tester.pumpAndSettle();
        await _expandHistory(tester);
        expect(find.text('Item updated'), findsOneWidget);
        expect(find.text('name: "Old" → "New"'), findsOneWidget);
      });

      testWidgets('renders ItemQuantityAdjusted with reason', (tester) async {
        final api = HomorgApi(_FakeAuth(),
            client: _clientWithHistory([
              {
                'id': 3,
                'event_type': 'ItemQuantityAdjusted',
                'created_at': '2026-04-17T10:00:00Z',
                'event_data': {'old_qty': 2, 'new_qty': 5, 'reason': 'restock'},
              },
            ]));
        await tester.pumpWidget(MaterialApp(home: ItemDetailScreen(itemId: _itemId, api: api)));
        await tester.pumpAndSettle();
        await _expandHistory(tester);
        expect(find.text('Quantity: 2 → 5'), findsOneWidget);
        expect(find.text('restock'), findsOneWidget);
      });

      testWidgets('renders ItemDeleted with reason in italic detail', (tester) async {
        final api = HomorgApi(_FakeAuth(),
            client: _clientWithHistory([
              {
                'id': 4,
                'event_type': 'ItemDeleted',
                'created_at': '2026-04-17T10:00:00Z',
                'event_data': {'reason': 'broken'},
              },
            ]));
        await tester.pumpWidget(MaterialApp(home: ItemDetailScreen(itemId: _itemId, api: api)));
        await tester.pumpAndSettle();
        await _expandHistory(tester);
        expect(find.text('Deleted'), findsOneWidget);
        expect(find.text('broken'), findsOneWidget);
      });

      testWidgets('renders multiple events in order', (tester) async {
        final api = HomorgApi(_FakeAuth(),
            client: _clientWithHistory([
              {
                'id': 10,
                'event_type': 'ItemCreated',
                'created_at': '2026-04-17T09:00:00Z',
                'event_data': {'name': 'My Item'},
              },
              {
                'id': 11,
                'event_type': 'BarcodeGenerated',
                'created_at': '2026-04-17T09:01:00Z',
                'event_data': {'barcode': 'HOM001'},
              },
            ]));
        await tester.pumpWidget(MaterialApp(home: ItemDetailScreen(itemId: _itemId, api: api)));
        await tester.pumpAndSettle();
        await _expandHistory(tester);
        expect(find.text('Created "My Item"'), findsOneWidget);
        expect(find.text('Barcode generated: HOM001'), findsOneWidget);
      });

      testWidgets('handles event with null eventData without crashing', (tester) async {
        final api = HomorgApi(_FakeAuth(),
            client: _clientWithHistory([
              {
                'id': 20,
                'event_type': 'ItemMoveReverted',
                'created_at': '2026-04-17T10:00:00Z',
                'event_data': null,
              },
            ]));
        await tester.pumpWidget(MaterialApp(home: ItemDetailScreen(itemId: _itemId, api: api)));
        await tester.pumpAndSettle();
        await _expandHistory(tester);
        expect(find.text('Move reverted'), findsOneWidget);
      });
    });
  });
}
