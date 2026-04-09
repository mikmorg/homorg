import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:homorg_camera/models/camera_models.dart';
import 'package:homorg_camera/services/api_service.dart';

void main() {
  final token = 'a' * 64;
  final connection = CameraConnection(baseUrl: 'http://localhost:8080', token: token);

  group('ApiService.getStatus', () {
    test('returns SessionStatus on 200', () async {
      final client = MockClient((request) async {
        expect(request.url.toString(), connection.statusUrl);
        expect(request.method, 'GET');
        return http.Response(
          jsonEncode({
            'session_id': 'sess-1',
            'active_container_id': 'cont-1',
            'active_item_id': 'item-1',
            'session_ended': false,
          }),
          200,
        );
      });

      final api = ApiService(connection, client: client);
      final status = await api.getStatus();
      expect(status.sessionId, 'sess-1');
      expect(status.activeContainerId, 'cont-1');
      expect(status.activeItemId, 'item-1');
      expect(status.sessionEnded, false);
    });

    test('returns SessionStatus with null optional fields', () async {
      final client = MockClient((request) async {
        return http.Response(
          jsonEncode({
            'session_id': 'sess-2',
            'active_container_id': null,
            'active_item_id': null,
            'session_ended': true,
          }),
          200,
        );
      });

      final api = ApiService(connection, client: client);
      final status = await api.getStatus();
      expect(status.sessionId, 'sess-2');
      expect(status.activeItemId, isNull);
      expect(status.sessionEnded, true);
    });

    test('throws ApiException with statusCode 401 on unauthorized', () async {
      final client = MockClient((request) async {
        return http.Response('Unauthorized', 401);
      });

      final api = ApiService(connection, client: client);
      expect(
        () => api.getStatus(),
        throwsA(
          isA<ApiException>()
              .having((e) => e.statusCode, 'statusCode', 401)
              .having((e) => e.message, 'message', contains('expired')),
        ),
      );
    });

    test('throws ApiException with statusCode 422 on no active item', () async {
      final client = MockClient((request) async {
        return http.Response('Unprocessable', 422);
      });

      final api = ApiService(connection, client: client);
      expect(
        () => api.getStatus(),
        throwsA(
          isA<ApiException>()
              .having((e) => e.statusCode, 'statusCode', 422)
              .having((e) => e.message, 'message', contains('No active item')),
        ),
      );
    });

    test('throws ApiException with body message on 400', () async {
      final client = MockClient((request) async {
        return http.Response(
          jsonEncode({'message': 'Invalid token format'}),
          400,
        );
      });

      final api = ApiService(connection, client: client);
      expect(
        () => api.getStatus(),
        throwsA(
          isA<ApiException>()
              .having((e) => e.statusCode, 'statusCode', 400)
              .having((e) => e.message, 'message', 'Invalid token format'),
        ),
      );
    });

    test('throws ApiException with fallback message on 400 with bad body', () async {
      final client = MockClient((request) async {
        return http.Response('not json', 400);
      });

      final api = ApiService(connection, client: client);
      expect(
        () => api.getStatus(),
        throwsA(
          isA<ApiException>()
              .having((e) => e.statusCode, 'statusCode', 400)
              .having((e) => e.message, 'message', 'Bad request'),
        ),
      );
    });

    test('throws ApiException with status code on 500', () async {
      final client = MockClient((request) async {
        return http.Response('Internal Server Error', 500);
      });

      final api = ApiService(connection, client: client);
      expect(
        () => api.getStatus(),
        throwsA(
          isA<ApiException>()
              .having((e) => e.statusCode, 'statusCode', 500)
              .having((e) => e.message, 'message', contains('500')),
        ),
      );
    });

    test('throws ApiException on SocketException', () async {
      final client = MockClient((request) {
        throw const SocketException('Connection refused');
      });

      final api = ApiService(connection, client: client);
      expect(
        () => api.getStatus(),
        throwsA(
          isA<ApiException>()
              .having((e) => e.message, 'message', contains('Cannot reach server')),
        ),
      );
    });

    test('throws ApiException on generic exception', () async {
      final client = MockClient((request) {
        throw Exception('Something unexpected');
      });

      final api = ApiService(connection, client: client);
      expect(
        () => api.getStatus(),
        throwsA(
          isA<ApiException>()
              .having((e) => e.message, 'message', 'Connection failed'),
        ),
      );
    });
  });

  group('ApiService.uploadImage', () {
    late File tempFile;

    setUpAll(() {
      tempFile = File('/tmp/test_upload_image.jpg');
      tempFile.writeAsBytesSync([0xFF, 0xD8, 0xFF, 0xE0]); // JPEG header bytes
    });

    tearDownAll(() {
      if (tempFile.existsSync()) tempFile.deleteSync();
    });

    test('returns UploadResult on 200', () async {
      final client = MockClient.streaming((request, bodyStream) async {
        expect(request.url.toString(), connection.uploadUrl);
        expect(request.method, 'POST');
        return http.StreamedResponse(
          Stream.value(utf8.encode(jsonEncode({
            'item_id': 'item-abc',
            'image_url': '/files/images/abc.jpg',
            'image_count': 5,
          }))),
          200,
        );
      });

      final api = ApiService(connection, client: client);
      final result = await api.uploadImage(tempFile);
      expect(result.itemId, 'item-abc');
      expect(result.imageUrl, '/files/images/abc.jpg');
      expect(result.imageCount, 5);
    });

    test('throws ApiException on 401', () async {
      final client = MockClient.streaming((request, bodyStream) async {
        return http.StreamedResponse(
          Stream.value(utf8.encode('Unauthorized')),
          401,
        );
      });

      final api = ApiService(connection, client: client);
      expect(
        () => api.uploadImage(tempFile),
        throwsA(
          isA<ApiException>().having((e) => e.statusCode, 'statusCode', 401),
        ),
      );
    });

    test('throws ApiException on 422', () async {
      final client = MockClient.streaming((request, bodyStream) async {
        return http.StreamedResponse(
          Stream.value(utf8.encode('Unprocessable')),
          422,
        );
      });

      final api = ApiService(connection, client: client);
      expect(
        () => api.uploadImage(tempFile),
        throwsA(
          isA<ApiException>()
              .having((e) => e.statusCode, 'statusCode', 422)
              .having((e) => e.message, 'message', contains('No active item')),
        ),
      );
    });

    test('throws ApiException on SocketException during upload', () async {
      final client = MockClient.streaming((request, bodyStream) {
        throw const SocketException('Connection reset');
      });

      final api = ApiService(connection, client: client);
      expect(
        () => api.uploadImage(tempFile),
        throwsA(
          isA<ApiException>()
              .having((e) => e.message, 'message', contains('Cannot reach server during upload')),
        ),
      );
    });

    test('throws ApiException on generic exception during upload', () async {
      final client = MockClient.streaming((request, bodyStream) {
        throw Exception('Boom');
      });

      final api = ApiService(connection, client: client);
      expect(
        () => api.uploadImage(tempFile),
        throwsA(
          isA<ApiException>()
              .having((e) => e.message, 'message', 'Upload connection failed'),
        ),
      );
    });
  });
}
