import 'dart:convert';
import 'dart:io';

import 'package:http/http.dart' as http;
import 'package:shared_preferences/shared_preferences.dart';

import '../models/auth.dart';

const _kServerUrl = 'server_url';
const _kAccessToken = 'access_token';
const _kRefreshToken = 'refresh_token';
const _kUserJson = 'auth_user';

class AuthService {
  final http.Client _client;

  String? _serverUrl;
  String? _accessToken;
  String? _refreshToken;
  AuthUser? _user;

  AuthService({http.Client? client}) : _client = client ?? http.Client();

  String? get serverUrl => _serverUrl;
  String? get accessToken => _accessToken;
  AuthUser? get user => _user;
  bool get isLoggedIn => _accessToken != null && _serverUrl != null;

  /// Load stored credentials from SharedPreferences.
  Future<void> loadStored() async {
    final prefs = await SharedPreferences.getInstance();
    _serverUrl = prefs.getString(_kServerUrl);
    _accessToken = prefs.getString(_kAccessToken);
    _refreshToken = prefs.getString(_kRefreshToken);
    final userJson = prefs.getString(_kUserJson);
    if (userJson != null) {
      _user = AuthUser.fromJson(jsonDecode(userJson) as Map<String, dynamic>);
    }
  }

  /// Authenticate with the server and store tokens.
  Future<AuthResponse> login(
    String serverUrl,
    String username,
    String password,
  ) async {
    final url = serverUrl.replaceAll(RegExp(r'/+$'), '');

    late http.Response response;
    try {
      response = await _client
          .post(
            Uri.parse('$url/api/v1/auth/login'),
            headers: {'Content-Type': 'application/json'},
            body: jsonEncode({
              'username': username,
              'password': password,
              'device_name': 'homorg-mobile',
            }),
          )
          .timeout(const Duration(seconds: 15));
    } on SocketException {
      throw const AuthException('Cannot reach server — check the URL and network');
    } on Exception {
      throw const AuthException('Connection failed');
    }

    if (response.statusCode == 200 || response.statusCode == 201) {
      final json = jsonDecode(response.body) as Map<String, dynamic>;
      final auth = AuthResponse.fromJson(json);

      _serverUrl = url;
      _accessToken = auth.accessToken;
      _refreshToken = auth.refreshToken;
      _user = auth.user;
      await _persist();

      return auth;
    }

    if (response.statusCode == 401) {
      throw const AuthException('Invalid username or password');
    }

    throw AuthException('Login failed (${response.statusCode})');
  }

  /// Refresh the access token using the stored refresh token.
  /// Returns true on success, false if re-login is needed.
  Future<bool> refresh() async {
    if (_serverUrl == null || _refreshToken == null) return false;

    late http.Response response;
    try {
      response = await _client
          .post(
            Uri.parse('$_serverUrl/api/v1/auth/refresh'),
            headers: {'Content-Type': 'application/json'},
            body: jsonEncode({'refresh_token': _refreshToken}),
          )
          .timeout(const Duration(seconds: 15));
    } catch (_) {
      return false;
    }

    if (response.statusCode == 200) {
      final json = jsonDecode(response.body) as Map<String, dynamic>;
      final auth = AuthResponse.fromJson(json);

      // Store both tokens atomically — the backend revokes the old refresh
      // token on rotation. If we only stored the access token, the next
      // refresh would trigger reuse detection and purge the token family.
      _accessToken = auth.accessToken;
      _refreshToken = auth.refreshToken;
      _user = auth.user;
      await _persist();
      return true;
    }

    // 401 means token family is dead — need re-login
    if (response.statusCode == 401) {
      await logout();
    }
    return false;
  }

  /// Clear all stored credentials.
  Future<void> logout() async {
    // Best-effort server-side logout
    if (_serverUrl != null && _accessToken != null && _refreshToken != null) {
      try {
        await _client
            .post(
              Uri.parse('$_serverUrl/api/v1/auth/logout'),
              headers: {
                'Content-Type': 'application/json',
                'Authorization': 'Bearer $_accessToken',
              },
              body: jsonEncode({'refresh_token': _refreshToken}),
            )
            .timeout(const Duration(seconds: 5));
      } catch (_) {
        // Ignore — we're logging out locally regardless
      }
    }

    _accessToken = null;
    _refreshToken = null;
    _user = null;
    // Keep _serverUrl so the login screen can pre-fill it

    final prefs = await SharedPreferences.getInstance();
    await prefs.remove(_kAccessToken);
    await prefs.remove(_kRefreshToken);
    await prefs.remove(_kUserJson);
  }

  Future<void> _persist() async {
    final prefs = await SharedPreferences.getInstance();
    if (_serverUrl != null) await prefs.setString(_kServerUrl, _serverUrl!);
    if (_accessToken != null) {
      await prefs.setString(_kAccessToken, _accessToken!);
    }
    if (_refreshToken != null) {
      await prefs.setString(_kRefreshToken, _refreshToken!);
    }
    if (_user != null) {
      await prefs.setString(_kUserJson, jsonEncode({
        'id': _user!.id,
        'username': _user!.username,
        'display_name': _user!.displayName,
        'role': _user!.role,
      }));
    }
  }
}

class AuthException implements Exception {
  final String message;
  const AuthException(this.message);

  @override
  String toString() => message;
}
