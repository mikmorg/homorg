class ScanSession {
  final String id;
  final String userId;
  final String? activeContainerId;
  final String? activeItemId;
  final String startedAt;
  final String? endedAt;
  final int itemsScanned;
  final int itemsCreated;
  final int itemsMoved;
  final int itemsErrored;
  final String? deviceId;
  final String? notes;
  final bool photoNeeded;

  const ScanSession({
    required this.id,
    required this.userId,
    this.activeContainerId,
    this.activeItemId,
    required this.startedAt,
    this.endedAt,
    this.itemsScanned = 0,
    this.itemsCreated = 0,
    this.itemsMoved = 0,
    this.itemsErrored = 0,
    this.deviceId,
    this.notes,
    this.photoNeeded = false,
  });

  factory ScanSession.fromJson(Map<String, dynamic> json) {
    return ScanSession(
      id: json['id'] as String,
      userId: json['user_id'] as String,
      activeContainerId: json['active_container_id'] as String?,
      activeItemId: json['active_item_id'] as String?,
      startedAt: json['started_at'] as String,
      endedAt: json['ended_at'] as String?,
      itemsScanned: json['items_scanned'] as int? ?? 0,
      itemsCreated: json['items_created'] as int? ?? 0,
      itemsMoved: json['items_moved'] as int? ?? 0,
      itemsErrored: json['items_errored'] as int? ?? 0,
      deviceId: json['device_id'] as String?,
      notes: json['notes'] as String?,
      photoNeeded: json['photo_needed'] as bool? ?? false,
    );
  }

  bool get isActive => endedAt == null;

  String get displayName {
    if (notes != null && notes!.isNotEmpty) return notes!;
    // Fall back to a timestamp-based label
    try {
      final dt = DateTime.parse(startedAt).toLocal();
      final h = dt.hour.toString().padLeft(2, '0');
      final m = dt.minute.toString().padLeft(2, '0');
      return '${dt.month}/${dt.day} $h:$m';
    } catch (_) {
      return 'Session';
    }
  }

  int get totalItems => itemsCreated + itemsMoved;
}

class CameraLink {
  final String token;
  final String sessionId;
  final String expiresAt;
  final String? deviceName;

  const CameraLink({
    required this.token,
    required this.sessionId,
    required this.expiresAt,
    this.deviceName,
  });

  factory CameraLink.fromJson(Map<String, dynamic> json) {
    return CameraLink(
      token: json['token'] as String,
      sessionId: json['session_id'] as String,
      expiresAt: json['expires_at'] as String,
      deviceName: json['device_name'] as String?,
    );
  }
}

/// Batch event types matching the backend StockerBatchEvent enum.
sealed class BatchEvent {
  Map<String, dynamic> toJson();
}

class SetContextEvent extends BatchEvent {
  final String containerId;

  SetContextEvent({required this.containerId});

  @override
  Map<String, dynamic> toJson() => {
        'type': 'set_context',
        'container_id': containerId,
        'scanned_at': DateTime.now().toUtc().toIso8601String(),
      };
}

class MoveItemEvent extends BatchEvent {
  final String itemId;

  MoveItemEvent({required this.itemId});

  @override
  Map<String, dynamic> toJson() => {
        'type': 'move_item',
        'item_id': itemId,
        'scanned_at': DateTime.now().toUtc().toIso8601String(),
      };
}

class CreateAndPlaceEvent extends BatchEvent {
  final String barcode;
  final String? name;
  final bool? isContainer;
  final String? containerTypeId;

  CreateAndPlaceEvent({
    required this.barcode,
    this.name,
    this.isContainer,
    this.containerTypeId,
  });

  @override
  Map<String, dynamic> toJson() {
    final m = <String, dynamic>{
      'type': 'create_and_place',
      'barcode': barcode,
      'scanned_at': DateTime.now().toUtc().toIso8601String(),
    };
    if (name != null) m['name'] = name;
    if (isContainer != null) m['is_container'] = isContainer;
    if (containerTypeId != null) m['container_type_id'] = containerTypeId;
    return m;
  }
}

/// Result from a batch submission.
class BatchResponse {
  final int processed;
  final List<BatchResult> results;
  final List<BatchError> errors;

  const BatchResponse({
    required this.processed,
    required this.results,
    required this.errors,
  });

  factory BatchResponse.fromJson(Map<String, dynamic> json) {
    return BatchResponse(
      processed: json['processed'] as int,
      results: (json['results'] as List<dynamic>)
          .map((e) => BatchResult.fromJson(e as Map<String, dynamic>))
          .toList(),
      errors: (json['errors'] as List<dynamic>)
          .map((e) => BatchError.fromJson(e as Map<String, dynamic>))
          .toList(),
    );
  }

  bool get hasErrors => errors.isNotEmpty;
}

class BatchResult {
  final String type;
  final int index;
  final String? itemId;
  final String? containerId;

  const BatchResult({
    required this.type,
    required this.index,
    this.itemId,
    this.containerId,
  });

  factory BatchResult.fromJson(Map<String, dynamic> json) {
    return BatchResult(
      type: json['type'] as String,
      index: json['index'] as int,
      itemId: json['item_id'] as String?,
      containerId: json['container_id'] as String?,
    );
  }
}

class BatchError {
  final int index;
  final String code;
  final String message;

  const BatchError({
    required this.index,
    required this.code,
    required this.message,
  });

  factory BatchError.fromJson(Map<String, dynamic> json) {
    return BatchError(
      index: json['index'] as int,
      code: json['code'] as String,
      message: json['message'] as String,
    );
  }
}
