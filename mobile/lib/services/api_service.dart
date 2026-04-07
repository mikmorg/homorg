import 'dart:convert';
import 'dart:io';

import 'package:http/http.dart' as http;

import '../models/camera_models.dart';

class ApiException implements Exception {
  final String message;
  final int? statusCode;

  const ApiException(this.message, {this.statusCode});

  @override
  String toString() => message;
}

class ApiService {
  final CameraConnection connection;

  ApiService(this.connection);

  Future<SessionStatus> getStatus() async {
    late http.Response response;
    try {
      response = await http
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
          await request.send().timeout(const Duration(seconds: 30));
    } on SocketException {
      throw const ApiException('Cannot reach server during upload');
    } on Exception {
      throw const ApiException('Upload connection failed');
    }

    final response = await http.Response.fromStream(streamed);

    if (response.statusCode == 200) {
      final json = jsonDecode(response.body) as Map<String, dynamic>;
      return UploadResult.fromJson(json);
    }
    _throwForStatus(response);
    throw const ApiException('Unexpected error');
  }

  void _throwForStatus(http.Response response) {
    switch (response.statusCode) {
      case 401:
        throw const ApiException('Token expired or invalid', statusCode: 401);
      case 400:
        String msg = 'Bad request';
        try {
          final body = jsonDecode(response.body) as Map<String, dynamic>;
          msg = (body['message'] as String?) ?? msg;
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
