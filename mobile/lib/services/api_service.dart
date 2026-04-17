import 'dart:convert';
import 'dart:io';

import 'package:http/http.dart' as http;

import '../models/camera_models.dart';

class ApiError implements Exception {
  final String message;
  final int? statusCode;

  const ApiError(this.message, {this.statusCode});

  @override
  String toString() => message;
}

// Alias for backward compatibility with ApiService callers
typedef ApiException = ApiError;

class ApiService {
  final CameraConnection connection;
  final http.Client _client;

  ApiService(this.connection, {http.Client? client})
      : _client = client ?? http.Client();

  Future<SessionStatus> getStatus() async {
    late http.Response response;
    try {
      response = await _client
          .get(Uri.parse(connection.statusUrl))
          .timeout(const Duration(seconds: 10));
    } on SocketException {
      throw const ApiException('Cannot reach server — check network and URL');
    } on Exception {
      throw const ApiException('Connection failed');
    }

    if (response.statusCode == 200) {
      final json = jsonDecode(response.body) as Map<String, dynamic>;
      return SessionStatus.fromJson(json);
    }
    _throwForStatus(response);
    throw const ApiException('Unexpected error');
  }

  Future<UploadResult> uploadImage(File imageFile) async {
    final request =
        http.MultipartRequest('POST', Uri.parse(connection.uploadUrl));

    request.files.add(await http.MultipartFile.fromPath('file', imageFile.path));

    late http.StreamedResponse streamed;
    try {
      streamed =
          await _client.send(request).timeout(const Duration(seconds: 30));
    } on SocketException {
      throw const ApiException('Cannot reach server during upload');
    } on Exception {
      throw const ApiException('Upload connection failed');
    }

    final response = await http.Response.fromStream(streamed);

    if (response.statusCode == 200 || response.statusCode == 201) {
      final json = jsonDecode(response.body) as Map<String, dynamic>;
      return UploadResult.fromJson(json);
    }
    _throwForStatus(response);
    throw const ApiException('Unexpected error');
  }

  /// Send a Bluetooth-scanner barcode to the session. Backend broadcasts it
  /// over the session's SSE stream as a `phone_scan` event; the web stocker
  /// UI handles routing. 204 No Content on success.
  Future<void> sendBarcode(String barcode) async {
    late http.Response response;
    try {
      response = await _client
          .post(
            Uri.parse(connection.scanUrl),
            headers: {'Content-Type': 'application/json'},
            body: jsonEncode({'barcode': barcode}),
          )
          .timeout(const Duration(seconds: 10));
    } on SocketException {
      throw const ApiException('Cannot reach server');
    } on Exception {
      throw const ApiException('Scan send failed');
    }

    if (response.statusCode == 204 || response.statusCode == 200) return;
    _throwForStatus(response);
  }

  void _throwForStatus(http.Response response) {
    switch (response.statusCode) {
      case 401:
        throw const ApiException(
          'Token expired or invalid — generate a new camera link',
          statusCode: 401,
        );
      case 400:
        String msg = 'Bad request';
        try {
          final body = jsonDecode(response.body) as Map<String, dynamic>;
          // Backend wraps errors as { "error": { "message": "..." } };
          // fall back to flat "message" for robustness.
          msg = ((body['error'] as Map<String, dynamic>?)?['message'] as String?) ??
              (body['message'] as String?) ??
              msg;
        } catch (_) {}
        throw ApiException(msg, statusCode: 400);
      case 422:
        throw const ApiException(
          'No active item — scan an item in the stocker first',
          statusCode: 422,
        );
      default:
        throw ApiException(
          'Server error (${response.statusCode})',
          statusCode: response.statusCode,
        );
    }
  }
}
