import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/testing.dart';
import 'package:http/http.dart' as http;

import 'package:homorg/models/item.dart';
import 'package:homorg/services/api_service.dart';
import 'package:homorg/services/auth_service.dart';
import 'package:homorg/services/homorg_api.dart';

class _FakeAuth extends Fake implements AuthService {
  @override
  String? get serverUrl => 'http://localhost:8080';

  @override
  String? get accessToken => 'test-token-123';
}

void main() {
  group('HomorgApi', () {
    test('getItem returns Item on 200', () async {
      final mockClient = MockClient((request) async {
        expect(request.url.toString(), 'http://localhost:8080/api/v1/items/test-id');
        expect(request.method, 'GET');
        expect(request.headers['Authorization'], 'Bearer test-token-123');

        return http.Response(
          jsonEncode({
            'id': 'test-id',
            'name': 'Test Item',
            'system_barcode': 'HOM001',
            'is_container': false,
            'is_deleted': false,
            'created_at': '2026-04-17T10:00:00Z',
            'updated_at': '2026-04-17T10:00:00Z',
          }),
          200,
        );
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);
      final item = await api.getItem('test-id');

      expect(item.id, 'test-id');
      expect(item.name, 'Test Item');
      expect(item.systemBarcode, 'HOM001');
      expect(item.isContainer, false);
      expect(item.isDeleted, false);
    });

    test('getItem throws ApiError on 404', () async {
      final mockClient = MockClient((request) async {
        return http.Response(
          jsonEncode({
            'error': {'message': 'Item not found'},
          }),
          404,
        );
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);
      expect(() => api.getItem('nonexistent'), throwsA(isA<ApiError>()));
    });

    test('getChildren returns List<ItemSummary>', () async {
      final mockClient = MockClient((request) async {
        expect(
          request.url.toString(),
          contains('/containers/parent-id/children'),
        );
        expect(request.headers['Authorization'], 'Bearer test-token-123');

        return http.Response(
          jsonEncode([
            {
              'id': 'child-1',
              'name': 'Child Item',
              'system_barcode': null,
              'is_container': false,
              'category': 'Electronics',
              'condition': 'good',
            },
          ]),
          200,
        );
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);
      final children = await api.getChildren('parent-id');

      expect(children, hasLength(1));
      expect(children[0].id, 'child-1');
      expect(children[0].name, 'Child Item');
      expect(children[0].category, 'Electronics');
    });

    test('getAncestors returns List<AncestorEntry>', () async {
      final mockClient = MockClient((request) async {
        expect(
          request.url.toString(),
          contains('/containers/item-id/ancestors'),
        );

        return http.Response(
          jsonEncode([
            {'id': 'root-id', 'name': 'Root'},
            {'id': 'parent-id', 'name': 'Parent'},
          ]),
          200,
        );
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);
      final ancestors = await api.getAncestors('item-id');

      expect(ancestors, hasLength(2));
      expect(ancestors[0].name, 'Root');
      expect(ancestors[1].name, 'Parent');
    });

    test('resolveBarcode returns SystemBarcode for system code', () async {
      final mockClient = MockClient((request) async {
        expect(
          request.url.toString(),
          contains('/barcodes/resolve/HOM001'),
        );

        return http.Response(
          jsonEncode({
            'type': 'system',
            'barcode': 'HOM001',
            'item_id': 'item-123',
          }),
          200,
        );
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);
      final result = await api.resolveBarcode('HOM001');

      expect(result, isA<SystemBarcode>());
      expect((result as SystemBarcode).itemId, 'item-123');
    });

    test('resolveBarcode returns Preset for preset code', () async {
      final mockClient = MockClient((request) async {
        return http.Response(
          jsonEncode({
            'type': 'preset',
            'barcode': 'PRE001',
            'is_container': true,
            'container_type_name': 'Box',
          }),
          200,
        );
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);
      final result = await api.resolveBarcode('PRE001');

      expect(result, isA<Preset>());
      expect((result as Preset).isContainer, true);
      expect(result.containerTypeName, 'Box');
    });

    test('resolveBarcode returns Unknown for unknown code', () async {
      final mockClient = MockClient((request) async {
        return http.Response(
          jsonEncode({
            'type': 'unknown',
            'value': 'UNKNOWN123',
          }),
          200,
        );
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);
      final result = await api.resolveBarcode('UNKNOWN123');

      expect(result, isA<Unknown>());
    });

    test('listContainerTypes returns List<ContainerType>', () async {
      final mockClient = MockClient((request) async {
        expect(request.url.toString(), contains('/container-types'));
        expect(request.headers['Authorization'], 'Bearer test-token-123');

        return http.Response(
          jsonEncode([
            {
              'id': 'type-1',
              'name': 'Plastic Bin',
              'description': 'Clear plastic storage container',
            },
            {
              'id': 'type-2',
              'name': 'Cardboard Box',
              'description': null,
            },
          ]),
          200,
        );
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);
      final types = await api.listContainerTypes();

      expect(types, hasLength(2));
      expect(types[0].name, 'Plastic Bin');
      expect(types[0].description, 'Clear plastic storage container');
      expect(types[1].name, 'Cardboard Box');
    });

    test('restoreItem sends POST and completes on 200', () async {
      final mockClient = MockClient((request) async {
        expect(request.url.toString(), contains('/items/item-id/restore'));
        expect(request.method, 'POST');
        expect(request.headers['Authorization'], 'Bearer test-token-123');

        return http.Response('', 204);
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);
      // Should not throw
      await api.restoreItem('item-id');
    });

    test('submitBatch returns BatchResponse with results', () async {
      final mockClient = MockClient((request) async {
        expect(
          request.url.toString(),
          contains('/stocker/sessions/session-id/batch'),
        );
        expect(request.method, 'POST');
        expect(request.headers['Authorization'], 'Bearer test-token-123');

        return http.Response(
          jsonEncode({
            'processed': 1,
            'results': [
              {
                'type': 'create_and_place',
                'index': 0,
                'item_id': 'new-item-id',
              },
            ],
            'errors': [],
          }),
          200,
        );
      });

      final api = HomorgApi(_FakeAuth(), client: mockClient);
      final response = await api.submitBatch(
        'session-id',
        [],
      );

      expect(response.processed, 1);
      expect(response.results, hasLength(1));
      expect(response.errors, isEmpty);
    });
  });
}
