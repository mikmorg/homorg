import 'package:flutter_test/flutter_test.dart';
import 'package:homorg/models/camera_models.dart';
import 'package:homorg/services/api_service.dart' show ApiException;

void main() {
  group('CameraConnection.tryParse', () {
    final validToken = 'a' * 64;

    test('parses a full upload URL', () {
      final url = 'http://192.168.1.10:8080/api/v1/stocker/camera/$validToken/upload';
      final conn = CameraConnection.tryParse(url);
      expect(conn, isNotNull);
      expect(conn!.baseUrl, 'http://192.168.1.10:8080');
      expect(conn.token, validToken);
    });

    test('parses a status URL', () {
      final url = 'http://example.com:3000/api/v1/stocker/camera/$validToken/status';
      final conn = CameraConnection.tryParse(url);
      expect(conn, isNotNull);
      expect(conn!.token, validToken);
    });

    test('parses https URL without explicit port', () {
      final url = 'https://homorg.example.com/api/v1/stocker/camera/$validToken/upload';
      final conn = CameraConnection.tryParse(url);
      expect(conn, isNotNull);
      expect(conn!.baseUrl, 'https://homorg.example.com');
    });

    test('parses http URL on port 80 — omits port', () {
      final url = 'http://myhost:80/api/v1/stocker/camera/$validToken/upload';
      final conn = CameraConnection.tryParse(url);
      expect(conn, isNotNull);
      expect(conn!.baseUrl, 'http://myhost');
    });

    test('parses https URL on port 443 — omits port', () {
      final url = 'https://myhost:443/api/v1/stocker/camera/$validToken/upload';
      final conn = CameraConnection.tryParse(url);
      expect(conn, isNotNull);
      expect(conn!.baseUrl, 'https://myhost');
    });

    test('lowercases token with uppercase hex', () {
      final upperToken = 'A' * 64;
      final url = 'http://host:8080/api/v1/stocker/camera/$upperToken/upload';
      final conn = CameraConnection.tryParse(url);
      expect(conn, isNotNull);
      expect(conn!.token, validToken);
    });

    test('trims whitespace from input', () {
      final url = '  http://host:8080/api/v1/stocker/camera/$validToken/upload  ';
      final conn = CameraConnection.tryParse(url);
      expect(conn, isNotNull);
    });

    test('returns null for empty string', () {
      expect(CameraConnection.tryParse(''), isNull);
    });

    test('returns null for garbage input', () {
      expect(CameraConnection.tryParse('not a url at all'), isNull);
    });

    test('returns null for URL without scheme', () {
      final url = 'host:8080/api/v1/stocker/camera/$validToken/upload';
      expect(CameraConnection.tryParse(url), isNull);
    });

    test('returns null when token is too short', () {
      final shortToken = 'a' * 63;
      final url = 'http://host:8080/api/v1/stocker/camera/$shortToken/upload';
      expect(CameraConnection.tryParse(url), isNull);
    });

    test('returns null when token is too long', () {
      final longToken = 'a' * 65;
      final url = 'http://host:8080/api/v1/stocker/camera/$longToken/upload';
      expect(CameraConnection.tryParse(url), isNull);
    });

    test('returns null when token contains non-hex characters', () {
      final badToken = 'g' * 64;
      final url = 'http://host:8080/api/v1/stocker/camera/$badToken/upload';
      expect(CameraConnection.tryParse(url), isNull);
    });

    test('returns null when no camera segment in path', () {
      final url = 'http://host:8080/api/v1/stocker/$validToken/upload';
      expect(CameraConnection.tryParse(url), isNull);
    });

    test('returns null when camera is the last segment', () {
      expect(CameraConnection.tryParse('http://host:8080/api/v1/stocker/camera'), isNull);
    });

    test('returns null when camera is the last segment with trailing slash', () {
      expect(CameraConnection.tryParse('http://host:8080/api/v1/stocker/camera/'), isNull);
    });
  });

  group('CameraConnection URL getters', () {
    final token = 'deadbeef' * 8; // 64 chars

    test('statusUrl is constructed correctly', () {
      final conn = CameraConnection(baseUrl: 'http://192.168.1.10:8080', token: token);
      expect(conn.statusUrl, 'http://192.168.1.10:8080/api/v1/stocker/camera/$token/status');
    });

    test('uploadUrl is constructed correctly', () {
      final conn = CameraConnection(baseUrl: 'https://example.com', token: token);
      expect(conn.uploadUrl, 'https://example.com/api/v1/stocker/camera/$token/upload');
    });
  });

  group('SessionStatus.fromJson', () {
    test('parses all fields present', () {
      final status = SessionStatus.fromJson({
        'session_id': 'sess-123',
        'active_container_id': 'cont-456',
        'active_item_id': 'item-789',
        'photo_needed': true,
        'session_ended': false,
      });
      expect(status.sessionId, 'sess-123');
      expect(status.activeContainerId, 'cont-456');
      expect(status.activeItemId, 'item-789');
      expect(status.photoNeeded, true);
      expect(status.sessionEnded, false);
    });

    test('parses with null optional fields', () {
      final status = SessionStatus.fromJson({
        'session_id': 'sess-123',
        'active_container_id': null,
        'active_item_id': null,
        'photo_needed': false,
        'session_ended': true,
      });
      expect(status.activeContainerId, isNull);
      expect(status.activeItemId, isNull);
      expect(status.photoNeeded, false);
      expect(status.sessionEnded, true);
    });

    test('parses with missing optional fields', () {
      final status = SessionStatus.fromJson({
        'session_id': 'sess-123',
        'session_ended': false,
      });
      expect(status.activeContainerId, isNull);
      expect(status.activeItemId, isNull);
      expect(status.photoNeeded, true, reason: 'defaults to true for backward compat');
    });

    test('throws on missing session_id', () {
      expect(
        () => SessionStatus.fromJson({'session_ended': false}),
        throwsA(isA<TypeError>()),
      );
    });

    test('throws on missing session_ended', () {
      expect(
        () => SessionStatus.fromJson({'session_id': 'x'}),
        throwsA(isA<TypeError>()),
      );
    });
  });

  group('UploadResult.fromJson', () {
    test('parses valid JSON', () {
      final result = UploadResult.fromJson({
        'item_id': 'item-abc',
        'image_url': '/files/images/abc.jpg',
        'image_count': 3,
      });
      expect(result.itemId, 'item-abc');
      expect(result.imageUrl, '/files/images/abc.jpg');
      expect(result.imageCount, 3);
    });

    test('throws on missing item_id', () {
      expect(
        () => UploadResult.fromJson({
          'image_url': '/files/images/abc.jpg',
          'image_count': 3,
        }),
        throwsA(isA<TypeError>()),
      );
    });

    test('throws on wrong type for image_count', () {
      expect(
        () => UploadResult.fromJson({
          'item_id': 'x',
          'image_url': 'y',
          'image_count': '3',
        }),
        throwsA(isA<TypeError>()),
      );
    });
  });

  group('ApiException', () {
    test('toString returns message', () {
      const e = ApiException('something went wrong', statusCode: 401);
      expect(e.toString(), 'something went wrong');
      expect(e.statusCode, 401);
    });

    test('statusCode defaults to null', () {
      const e = ApiException('oops');
      expect(e.statusCode, isNull);
    });
  });
}
