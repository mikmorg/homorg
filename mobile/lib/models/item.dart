class HistoryEvent {
  final int id;
  final String eventType;
  final String createdAt;
  final Map<String, dynamic>? eventData;
  final String? aggregateId;

  const HistoryEvent({
    required this.id,
    required this.eventType,
    required this.createdAt,
    this.eventData,
    this.aggregateId,
  });

  factory HistoryEvent.fromJson(Map<String, dynamic> json) => HistoryEvent(
        id: json['id'] as int,
        eventType: json['event_type'] as String,
        createdAt: json['created_at'] as String,
        eventData: json['event_data'] as Map<String, dynamic>?,
        aggregateId: json['aggregate_id'] as String?,
      );
}

class ExternalCodeEntry {
  final String codeType;
  final String value;

  const ExternalCodeEntry({required this.codeType, required this.value});

  factory ExternalCodeEntry.fromJson(Map<String, dynamic> json) {
    return ExternalCodeEntry(
      codeType: json['type'] as String? ?? json['code_type'] as String? ?? '',
      value: json['value'] as String,
    );
  }
}

class ItemImage {
  final String path;
  final String? caption;
  final int? order;

  const ItemImage({required this.path, this.caption, this.order});

  factory ItemImage.fromJson(Map<String, dynamic> json) {
    return ItemImage(
      path: json['path'] as String,
      caption: json['caption'] as String?,
      order: json['order'] as int?,
    );
  }
}

class AncestorEntry {
  final String id;
  final String? name;

  const AncestorEntry({required this.id, this.name});

  factory AncestorEntry.fromJson(Map<String, dynamic> json) {
    return AncestorEntry(
      id: json['id'] as String,
      name: json['name'] as String?,
    );
  }
}

class Item {
  final String id;
  final String? systemBarcode;
  final String? name;
  final String? description;
  final String? category;
  final List<String> tags;
  final bool isContainer;
  final String? containerPath;
  final String? parentId;
  final String? condition;
  final List<ItemImage> images;
  final List<ExternalCodeEntry> externalCodes;
  final bool isFungible;
  final int? fungibleQuantity;
  final String? fungibleUnit;
  final List<AncestorEntry> ancestors;
  final String? acquisitionDate;
  final double? acquisitionCost;
  final double? currentValue;
  final String? warrantyExpiry;
  final String? currency;
  final double? weightGrams;
  final Map<String, dynamic> metadata;
  final String createdAt;
  final String updatedAt;

  const Item({
    required this.id,
    this.systemBarcode,
    this.name,
    this.description,
    this.category,
    this.tags = const [],
    this.isContainer = false,
    this.containerPath,
    this.parentId,
    this.condition,
    this.images = const [],
    this.externalCodes = const [],
    this.isFungible = false,
    this.fungibleQuantity,
    this.fungibleUnit,
    this.ancestors = const [],
    this.acquisitionDate,
    this.acquisitionCost,
    this.currentValue,
    this.warrantyExpiry,
    this.currency,
    this.weightGrams,
    this.metadata = const {},
    required this.createdAt,
    required this.updatedAt,
  });

  factory Item.fromJson(Map<String, dynamic> json) {
    return Item(
      id: json['id'] as String,
      systemBarcode: json['system_barcode'] as String?,
      name: json['name'] as String?,
      description: json['description'] as String?,
      category: json['category'] as String?,
      tags: (json['tags'] as List<dynamic>?)?.cast<String>() ?? [],
      isContainer: json['is_container'] as bool? ?? false,
      containerPath: json['container_path'] as String?,
      parentId: json['parent_id'] as String?,
      condition: json['condition'] as String?,
      images: (json['images'] as List<dynamic>?)
              ?.map((e) => ItemImage.fromJson(e as Map<String, dynamic>))
              .toList() ??
          [],
      externalCodes: _parseExternalCodes(json['external_codes']),
      isFungible: json['is_fungible'] as bool? ?? false,
      fungibleQuantity: json['fungible_quantity'] as int?,
      fungibleUnit: json['fungible_unit'] as String?,
      ancestors: (json['ancestors'] as List<dynamic>?)
              ?.map((e) => AncestorEntry.fromJson(e as Map<String, dynamic>))
              .toList() ??
          [],
      acquisitionDate: json['acquisition_date'] as String?,
      acquisitionCost: _toDouble(json['acquisition_cost']),
      currentValue: _toDouble(json['current_value']),
      warrantyExpiry: json['warranty_expiry'] as String?,
      currency: json['currency'] as String?,
      weightGrams: _toDouble(json['weight_grams']),
      metadata: _parseMetadata(json['metadata']),
      createdAt: json['created_at'] as String,
      updatedAt: json['updated_at'] as String,
    );
  }

  static List<ExternalCodeEntry> _parseExternalCodes(dynamic value) {
    if (value == null) return [];
    if (value is List) {
      return value
          .whereType<Map<String, dynamic>>()
          .map((e) => ExternalCodeEntry.fromJson(e))
          .toList();
    }
    return [];
  }

  static double? _toDouble(dynamic value) {
    if (value == null) return null;
    if (value is num) return value.toDouble();
    if (value is String) return double.tryParse(value);
    return null;
  }

  static Map<String, dynamic> _parseMetadata(dynamic value) {
    if (value == null) return {};
    if (value is Map<String, dynamic>) return value;
    if (value is Map) return Map<String, dynamic>.from(value);
    return {};
  }

  String get displayName => name ?? systemBarcode ?? 'Unnamed';

  String get locationBreadcrumb {
    if (ancestors.isEmpty) return '';
    return ancestors.map((a) => a.name ?? '?').join(' › ');
  }
}

/// Result from GET /barcodes/resolve/{code}
sealed class BarcodeResolution {
  const BarcodeResolution();

  factory BarcodeResolution.fromJson(Map<String, dynamic> json) {
    switch (json['type'] as String) {
      case 'system':
        return SystemBarcode(
          barcode: json['barcode'] as String,
          itemId: json['item_id'] as String,
        );
      case 'external':
        return ExternalCode(
          codeType: json['code_type'] as String,
          value: json['value'] as String,
          itemIds: (json['item_ids'] as List<dynamic>?)?.cast<String>() ?? [],
        );
      case 'preset':
        return Preset(
          barcode: json['barcode'] as String,
          isContainer: json['is_container'] as bool,
          containerTypeName: json['container_type_name'] as String?,
        );
      case 'unknown_system':
        return UnknownSystem(barcode: json['barcode'] as String);
      case 'unknown':
        return Unknown(value: json['value'] as String);
      default:
        return Unknown(value: json['value']?.toString() ?? json['barcode']?.toString() ?? '');
    }
  }
}

class SystemBarcode extends BarcodeResolution {
  final String barcode;
  final String itemId;
  const SystemBarcode({required this.barcode, required this.itemId});
}

class ExternalCode extends BarcodeResolution {
  final String codeType;
  final String value;
  final List<String> itemIds;
  const ExternalCode({required this.codeType, required this.value, required this.itemIds});
}

class Preset extends BarcodeResolution {
  final String barcode;
  final bool isContainer;
  final String? containerTypeName;
  const Preset({required this.barcode, required this.isContainer, this.containerTypeName});
}

class UnknownSystem extends BarcodeResolution {
  final String barcode;
  const UnknownSystem({required this.barcode});
}

class Unknown extends BarcodeResolution {
  final String value;
  const Unknown({required this.value});
}

class ItemSummary {
  final String id;
  final String? systemBarcode;
  final String? name;
  final String? category;
  final bool isContainer;
  final String? containerPath;
  final String? parentName;
  final String? condition;

  const ItemSummary({
    required this.id,
    this.systemBarcode,
    this.name,
    this.category,
    this.isContainer = false,
    this.containerPath,
    this.parentName,
    this.condition,
  });

  factory ItemSummary.fromJson(Map<String, dynamic> json) {
    return ItemSummary(
      id: json['id'] as String,
      systemBarcode: json['system_barcode'] as String?,
      name: json['name'] as String?,
      category: json['category'] as String?,
      isContainer: json['is_container'] as bool? ?? false,
      containerPath: json['container_path'] as String?,
      parentName: json['parent_name'] as String?,
      condition: json['condition'] as String?,
    );
  }

  String get displayName => name ?? systemBarcode ?? 'Unnamed';
}
