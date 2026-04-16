import 'dart:convert';
import 'dart:io';

import 'package:http/http.dart' as http;

import '../models/item.dart';
import '../models/session.dart';
import 'auth_service.dart';

/// JWT-authenticated API client for general Homorg endpoints.
class HomorgApi {
  final AuthService _auth;
  final http.Client _client;

  HomorgApi(this._auth, {http.Client? client})
      : _client = client ?? http.Client();

  String get _baseUrl => _auth.serverUrl!;

  /// Base URL for building web links (same origin as the API).
  String get webUrl => _baseUrl;

  Map<String, String> get _headers => {
        'Authorization': 'Bearer ${_auth.accessToken}',
        'Content-Type': 'application/json',
      };

  /// Resolve a barcode (system, external, preset, or unknown).
  Future<BarcodeResolution> resolveBarcode(String code) async {
    final encoded = Uri.encodeComponent(code);
    return _get<BarcodeResolution>(
      '/api/v1/barcodes/resolve/$encoded',
      (json) => BarcodeResolution.fromJson(json),
    );
  }

  /// Get full item detail by ID.
  Future<Item> getItem(String id) async {
    return _get<Item>('/api/v1/items/$id', (json) => Item.fromJson(json));
  }

  /// Search items by query string.
  Future<List<ItemSummary>> search(String query) async {
    final encoded = Uri.encodeQueryComponent(query);
    final response = await _request('GET', '/api/v1/search?q=$encoded');
    final list = jsonDecode(response.body) as List<dynamic>;
    return list
        .map((e) => ItemSummary.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  /// Move an item into a container.
  Future<void> moveItem(String itemId, String containerId) async {
    await _request(
      'POST',
      '/api/v1/items/$itemId/move',
      body: jsonEncode({'container_id': containerId}),
    );
  }

  /// Upload an image for an item (multipart).
  Future<void> uploadImage(String itemId, File imageFile) async {
    final uri = Uri.parse('$_baseUrl/api/v1/items/$itemId/images');
    final request = http.MultipartRequest('POST', uri);
    request.headers['Authorization'] = 'Bearer ${_auth.accessToken}';
    request.files.add(await http.MultipartFile.fromPath('file', imageFile.path));

    late http.StreamedResponse streamed;
    try {
      streamed =
          await _client.send(request).timeout(const Duration(seconds: 30));
    } on SocketException {
      throw const ApiError('Cannot reach server');
    } catch (_) {
      throw const ApiError('Upload failed');
    }

    if (streamed.statusCode == 401) {
      final refreshed = await _auth.refresh();
      if (!refreshed) throw const ApiError('Session expired — please log in again');

      // Retry with new token
      final retry = http.MultipartRequest('POST', uri);
      retry.headers['Authorization'] = 'Bearer ${_auth.accessToken}';
      retry.files
          .add(await http.MultipartFile.fromPath('file', imageFile.path));
      final retryResp =
          await _client.send(retry).timeout(const Duration(seconds: 30));
      if (retryResp.statusCode != 200 && retryResp.statusCode != 201) {
        throw ApiError('Upload failed (${retryResp.statusCode})');
      }
      return;
    }

    if (streamed.statusCode != 200 && streamed.statusCode != 201) {
      throw ApiError('Upload failed (${streamed.statusCode})');
    }
  }

  /// Build a full image URL from the stored path (e.g. "/files/uuid/img.jpg").
  /// The backend stores the complete URL path including the /files/ prefix.
  String imageUrl(String path) => '$_baseUrl$path';

  // ── Stocker session management ──────────────────────────────────────

  /// List the current user's scan sessions.
  Future<List<ScanSession>> listSessions({int limit = 20}) async {
    final response = await _request('GET', '/api/v1/stocker/sessions?limit=$limit');
    final list = jsonDecode(response.body) as List<dynamic>;
    return list
        .map((e) => ScanSession.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  /// Get a single session.
  Future<ScanSession> getSession(String id) async {
    return _get<ScanSession>(
      '/api/v1/stocker/sessions/$id',
      (json) => ScanSession.fromJson(json),
    );
  }

  /// Create a new stocker session.
  Future<ScanSession> createSession({
    String? notes,
    String? deviceId,
    String? initialContainerId,
  }) async {
    final body = <String, dynamic>{};
    if (notes != null) body['notes'] = notes;
    if (deviceId != null) body['device_id'] = deviceId;
    if (initialContainerId != null) {
      body['initial_container_id'] = initialContainerId;
    }
    final response = await _request(
      'POST',
      '/api/v1/stocker/sessions',
      body: jsonEncode(body),
    );
    final json = jsonDecode(response.body) as Map<String, dynamic>;
    return ScanSession.fromJson(json);
  }

  /// End a stocker session.
  Future<ScanSession> endSession(String id) async {
    final response = await _request('PUT', '/api/v1/stocker/sessions/$id/end');
    final json = jsonDecode(response.body) as Map<String, dynamic>;
    return ScanSession.fromJson(json);
  }

  /// Submit a batch of stocker events.
  Future<BatchResponse> submitBatch(
    String sessionId,
    List<BatchEvent> events,
  ) async {
    final response = await _request(
      'POST',
      '/api/v1/stocker/sessions/$sessionId/batch',
      body: jsonEncode({'events': events.map((e) => e.toJson()).toList()}),
    );
    final json = jsonDecode(response.body) as Map<String, dynamic>;
    return BatchResponse.fromJson(json);
  }

  /// Create a camera link for a session.
  Future<CameraLink> createCameraLink(
    String sessionId, {
    String? deviceName,
    int? expiresInHours,
  }) async {
    final body = <String, dynamic>{};
    if (deviceName != null) body['device_name'] = deviceName;
    if (expiresInHours != null) body['expires_in_hours'] = expiresInHours;
    final response = await _request(
      'POST',
      '/api/v1/stocker/sessions/$sessionId/camera-links',
      body: jsonEncode(body),
    );
    final json = jsonDecode(response.body) as Map<String, dynamic>;
    return CameraLink.fromJson(json);
  }

  /// Search containers only.
  Future<List<ItemSummary>> searchContainers(String query) async {
    final encoded = Uri.encodeQueryComponent(query);
    final response =
        await _request('GET', '/api/v1/search?q=$encoded&is_container=true');
    final list = jsonDecode(response.body) as List<dynamic>;
    return list
        .map((e) => ItemSummary.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  // ── Item CRUD ────────────────────────────────────────────────────────

  /// Create a new item.
  Future<Item> createItem(Map<String, dynamic> body) async {
    final response = await _request(
      'POST',
      '/api/v1/items',
      body: jsonEncode(body),
    );
    final json = jsonDecode(response.body) as Map<String, dynamic>;
    // Backend returns StoredEvent with event.data containing the item.
    // Re-fetch the item for a clean Item object.
    final itemId = json['item_id'] as String? ?? json['data']?['id'] as String?;
    if (itemId != null) return getItem(itemId);
    throw const ApiError('Create succeeded but no item ID returned');
  }

  /// Partially update an item. Only include changed fields in the map.
  Future<void> updateItem(String id, Map<String, dynamic> body) async {
    await _request('PUT', '/api/v1/items/$id', body: jsonEncode(body));
  }

  /// Soft-delete an item.
  Future<void> deleteItem(String id) async {
    await _request('DELETE', '/api/v1/items/$id');
  }

  /// Restore a soft-deleted item.
  Future<void> restoreItem(String id) async {
    await _request('POST', '/api/v1/items/$id/restore');
  }

  // ── Image management ────────────────────────────────────────────────

  /// Remove an image by index.
  Future<void> deleteImage(String itemId, int imageIndex) async {
    await _request('DELETE', '/api/v1/items/$itemId/images/$imageIndex');
  }

  // ── Barcode management ──────────────────────────────────────────────

  /// Assign a system barcode to an item.
  Future<void> assignBarcode(String itemId, String barcode) async {
    await _request(
      'POST',
      '/api/v1/items/$itemId/barcode',
      body: jsonEncode({'barcode': barcode}),
    );
  }

  /// Generate a new system barcode.
  Future<String> generateBarcode() async {
    final response = await _request('POST', '/api/v1/barcodes/generate');
    final json = jsonDecode(response.body) as Map<String, dynamic>;
    return json['barcode'] as String;
  }

  /// Add an external code (UPC, EAN, etc.) to an item.
  Future<void> addExternalCode(
      String itemId, String codeType, String value) async {
    await _request(
      'POST',
      '/api/v1/items/$itemId/external-codes',
      body: jsonEncode({'code_type': codeType, 'value': value}),
    );
  }

  /// Remove an external code from an item.
  Future<void> removeExternalCode(
      String itemId, String codeType, String value) async {
    final encodedType = Uri.encodeComponent(codeType);
    final encodedValue = Uri.encodeComponent(value);
    await _request(
      'DELETE',
      '/api/v1/items/$itemId/external-codes/$encodedType/$encodedValue',
    );
  }

  // ── Container browsing ──────────────────────────────────────────────

  /// Get direct children of a container (supports cursor pagination).
  Future<List<ItemSummary>> getChildren(String containerId,
      {int limit = 50, String? cursor}) async {
    var url = '/api/v1/containers/$containerId/children?limit=$limit';
    if (cursor != null) url += '&cursor=${Uri.encodeComponent(cursor)}';
    final response = await _request('GET', url);
    final list = jsonDecode(response.body) as List<dynamic>;
    return list
        .map((e) => ItemSummary.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  /// Get ancestor breadcrumb for a container.
  Future<List<AncestorEntry>> getAncestors(String containerId) async {
    final response =
        await _request('GET', '/api/v1/containers/$containerId/ancestors');
    final list = jsonDecode(response.body) as List<dynamic>;
    return list
        .map((e) => AncestorEntry.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  // ── Item quantity & history ─────────────────────────────────────────

  /// Adjust fungible quantity. POST /api/v1/items/{id}/quantity
  Future<void> adjustQuantity(String itemId, int newQuantity,
      {String? reason}) async {
    final body = <String, dynamic>{'new_quantity': newQuantity};
    if (reason != null && reason.isNotEmpty) body['reason'] = reason;
    await _request('POST', '/api/v1/items/$itemId/quantity',
        body: jsonEncode(body));
  }

  /// Get event history for an item. GET /api/v1/items/{id}/history
  Future<List<HistoryEvent>> getItemHistory(String itemId,
      {int limit = 50}) async {
    final response =
        await _request('GET', '/api/v1/items/$itemId/history?limit=$limit');
    final list = jsonDecode(response.body) as List<dynamic>;
    return list
        .map((e) => HistoryEvent.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  // ── Taxonomy ───────────────────────────────────────────────────────

  /// List all known categories.
  Future<List<String>> listCategories() async {
    final response = await _request('GET', '/api/v1/categories');
    final list = jsonDecode(response.body) as List<dynamic>;
    return list
        .map((e) => (e as Map<String, dynamic>)['name'] as String)
        .toList();
  }

  /// List all known tags.
  Future<List<String>> listTags() async {
    final response = await _request('GET', '/api/v1/tags');
    final list = jsonDecode(response.body) as List<dynamic>;
    return list
        .map((e) => (e as Map<String, dynamic>)['name'] as String)
        .toList();
  }

  // ── Internal helpers ─────────────────────────────────────────────────

  Future<T> _get<T>(
      String path, T Function(Map<String, dynamic>) fromJson) async {
    final response = await _request('GET', path);
    final json = jsonDecode(response.body) as Map<String, dynamic>;
    return fromJson(json);
  }

  /// Make a request with automatic 401 retry via token refresh.
  Future<http.Response> _request(String method, String path,
      {String? body}) async {
    var response = await _rawRequest(method, path, body: body);

    if (response.statusCode == 401) {
      final refreshed = await _auth.refresh();
      if (!refreshed) {
        throw const ApiError('Session expired — please log in again');
      }
      response = await _rawRequest(method, path, body: body);
    }

    if (response.statusCode >= 400) {
      String msg = 'Request failed (${response.statusCode})';
      try {
        final json = jsonDecode(response.body) as Map<String, dynamic>;
        msg = ((json['error'] as Map<String, dynamic>?)?['message']
                as String?) ??
            (json['message'] as String?) ??
            msg;
      } catch (_) {}
      throw ApiError(msg);
    }

    return response;
  }

  Future<http.Response> _rawRequest(String method, String path,
      {String? body}) async {
    final uri = Uri.parse('$_baseUrl$path');
    try {
      switch (method) {
        case 'GET':
          return await _client
              .get(uri, headers: _headers)
              .timeout(const Duration(seconds: 15));
        case 'POST':
          return await _client
              .post(uri, headers: _headers, body: body)
              .timeout(const Duration(seconds: 15));
        case 'PUT':
          return await _client
              .put(uri, headers: _headers, body: body)
              .timeout(const Duration(seconds: 15));
        case 'DELETE':
          return await _client
              .delete(uri, headers: _headers)
              .timeout(const Duration(seconds: 15));
        default:
          throw ApiError('Unsupported method: $method');
      }
    } on SocketException {
      throw const ApiError('Cannot reach server — check network');
    } on Exception {
      throw const ApiError('Connection failed');
    }
  }
}

class ApiError implements Exception {
  final String message;
  const ApiError(this.message);

  @override
  String toString() => message;
}
