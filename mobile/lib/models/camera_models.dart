class SessionStatus {
  final String sessionId;
  final String? activeContainerId;
  final String? activeItemId;
  final bool photoNeeded;
  final bool sessionEnded;

  const SessionStatus({
    required this.sessionId,
    this.activeContainerId,
    this.activeItemId,
    this.photoNeeded = true,
    required this.sessionEnded,
  });

  factory SessionStatus.fromJson(Map<String, dynamic> json) {
    return SessionStatus(
      sessionId: json['session_id'] as String,
      activeContainerId: json['active_container_id'] as String?,
      activeItemId: json['active_item_id'] as String?,
      photoNeeded: json['photo_needed'] as bool? ?? true,
      sessionEnded: json['session_ended'] as bool,
    );
  }
}

class UploadResult {
  final String itemId;
  final String imageUrl;
  final int imageCount;

  const UploadResult({
    required this.itemId,
    required this.imageUrl,
    required this.imageCount,
  });

  factory UploadResult.fromJson(Map<String, dynamic> json) {
    return UploadResult(
      itemId: json['item_id'] as String,
      imageUrl: json['image_url'] as String,
      imageCount: json['image_count'] as int,
    );
  }
}

/// Parsed representation of a camera upload URL.
///
/// The stocker page displays: {origin}/api/v1/stocker/camera/{token}/upload
/// Status endpoint is:         {origin}/api/v1/stocker/camera/{token}/status
class CameraConnection {
  final String baseUrl; // e.g. http://192.168.1.10:8080
  final String token; // 64-char hex

  const CameraConnection({required this.baseUrl, required this.token});

  String get statusUrl => '$baseUrl/api/v1/stocker/camera/$token/status';
  String get uploadUrl => '$baseUrl/api/v1/stocker/camera/$token/upload';
  String get scanUrl => '$baseUrl/api/v1/stocker/camera/$token/scan';

  /// Parses the upload URL shown on the stocker camera link panel.
  /// Accepts any URL that contains /camera/{64-hex-token}/ in the path.
  static CameraConnection? tryParse(String input) {
    final uri = Uri.tryParse(input.trim());
    if (uri == null || !uri.hasScheme || !uri.hasAuthority) return null;

    final segments = uri.pathSegments;
    final cameraIdx = segments.indexOf('camera');
    if (cameraIdx == -1 || cameraIdx + 1 >= segments.length) return null;

    final token = segments[cameraIdx + 1].toLowerCase();
    if (token.length != 64 || !RegExp(r'^[0-9a-f]+$').hasMatch(token)) {
      return null;
    }

    final port = (uri.port != 0 && uri.port != 80 && uri.port != 443)
        ? ':${uri.port}'
        : '';
    final baseUrl = '${uri.scheme}://${uri.host}$port';
    return CameraConnection(baseUrl: baseUrl, token: token);
  }
}
