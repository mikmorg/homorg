class AuthUser {
  final String id;
  final String username;
  final String? displayName;
  final String role;

  const AuthUser({
    required this.id,
    required this.username,
    this.displayName,
    required this.role,
  });

  factory AuthUser.fromJson(Map<String, dynamic> json) {
    return AuthUser(
      id: json['id'] as String,
      username: json['username'] as String,
      displayName: json['display_name'] as String?,
      role: json['role'] as String,
    );
  }
}

class AuthResponse {
  final String accessToken;
  final String refreshToken;
  final int expiresIn;
  final AuthUser user;

  const AuthResponse({
    required this.accessToken,
    required this.refreshToken,
    required this.expiresIn,
    required this.user,
  });

  factory AuthResponse.fromJson(Map<String, dynamic> json) {
    return AuthResponse(
      accessToken: json['access_token'] as String,
      refreshToken: json['refresh_token'] as String,
      expiresIn: json['expires_in'] as int,
      user: AuthUser.fromJson(json['user'] as Map<String, dynamic>),
    );
  }
}
