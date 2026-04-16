import 'dart:async';
import 'dart:io';

import 'package:camera/camera.dart';
import 'package:flutter/material.dart';

import '../models/item.dart';
import '../services/homorg_api.dart';
import 'camera_capture_screen.dart';

const _conditions = ['new', 'like_new', 'good', 'fair', 'poor', 'broken'];

String _conditionLabel(String c) => c.replaceAll('_', ' ');

String _formatDate(String iso) {
  final dt = DateTime.tryParse(iso);
  if (dt == null) return iso;
  return '${dt.year}-${dt.month.toString().padLeft(2, '0')}-${dt.day.toString().padLeft(2, '0')}';
}

String _formatDateTime(String iso) {
  final dt = DateTime.tryParse(iso);
  if (dt == null) return iso;
  return '${dt.year}-${dt.month.toString().padLeft(2, '0')}-${dt.day.toString().padLeft(2, '0')} '
      '${dt.hour.toString().padLeft(2, '0')}:${dt.minute.toString().padLeft(2, '0')}';
}

String _formatCurrency(double value, String? currency) {
  final c = currency ?? 'USD';
  return '${value.toStringAsFixed(2)} $c';
}

class ItemDetailScreen extends StatefulWidget {
  final HomorgApi api;
  final String itemId;

  const ItemDetailScreen({super.key, required this.api, required this.itemId});

  @override
  State<ItemDetailScreen> createState() => _ItemDetailScreenState();
}

class _ItemDetailScreenState extends State<ItemDetailScreen> {
  Item? _item;
  bool _loading = true;
  String? _error;
  int? _lightboxIndex;
  bool _modified = false;

  // Container children (loaded if item is a container)
  List<ItemSummary>? _children;
  bool _loadingChildren = false;
  String? _childrenCursor;
  bool _hasMoreChildren = false;
  bool _loadingMoreChildren = false;

  // History (lazy-loaded)
  bool _historyExpanded = false;
  List<HistoryEvent>? _history;
  bool _loadingHistory = false;

  @override
  void initState() {
    super.initState();
    _loadItem();
  }

  Future<void> _loadItem() async {
    setState(() {
      _loading = true;
      _error = null;
    });

    try {
      final item = await widget.api.getItem(widget.itemId);
      if (!mounted) return;
      setState(() {
        _item = item;
        _loading = false;
      });
      if (item.isContainer) _loadChildren();
    } on ApiError catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e.message;
        _loading = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _error = 'Failed to load item';
        _loading = false;
      });
    }
  }

  Future<void> _loadChildren() async {
    setState(() => _loadingChildren = true);
    try {
      final children = await widget.api.getChildren(widget.itemId);
      if (mounted) {
        setState(() {
          _children = children;
          _hasMoreChildren = children.length == 50;
          _childrenCursor = children.isNotEmpty ? children.last.id : null;
          _loadingChildren = false;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _loadingChildren = false);
    }
  }

  Future<void> _loadMoreChildren() async {
    if (_loadingMoreChildren || _childrenCursor == null) return;
    setState(() => _loadingMoreChildren = true);
    try {
      final more = await widget.api.getChildren(
          widget.itemId, cursor: _childrenCursor);
      if (mounted) {
        setState(() {
          _children = [...?_children, ...more];
          _hasMoreChildren = more.length == 50;
          _childrenCursor = more.isNotEmpty ? more.last.id : null;
          _loadingMoreChildren = false;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _loadingMoreChildren = false);
    }
  }

  // ── History ────────────────────────────────────────────────────────

  Future<void> _loadHistory() async {
    if (_loadingHistory) return;
    setState(() => _loadingHistory = true);
    try {
      final events = await widget.api.getItemHistory(widget.itemId);
      if (mounted) {
        setState(() {
          _history = events;
          _loadingHistory = false;
        });
      }
    } catch (_) {
      if (mounted) {
        setState(() => _loadingHistory = false);
      }
    }
  }

  // ── Photo ─────────────────────────────────────────────────────────

  Future<void> _takePhoto() async {
    XFile? photo;
    try {
      photo = await Navigator.of(context).push<XFile>(
        MaterialPageRoute(
          builder: (_) => const CameraCaptureScreen(),
          fullscreenDialog: true,
        ),
      );
    } catch (_) {
      if (mounted) _snack('Camera access denied');
      return;
    }

    if (photo == null || !mounted) return;
    _snack('Uploading photo…');

    try {
      await widget.api.uploadImage(widget.itemId, File(photo.path));
      if (!mounted) return;
      _snack('Photo uploaded');
      _modified = true;
      _loadItem();
    } on ApiError catch (e) {
      if (mounted) _snack('Upload failed: ${e.message}');
    }
  }

  Future<void> _deleteImage(int index) async {
    final confirm = await _confirmDialog(
      'Delete Image',
      'Remove this image permanently?',
    );
    if (confirm != true || !mounted) return;

    try {
      await widget.api.deleteImage(widget.itemId, index);
      if (!mounted) return;
      _snack('Image deleted');
      _modified = true;
      setState(() => _lightboxIndex = null);
      _loadItem();
    } on ApiError catch (e) {
      if (mounted) _snack('Failed: ${e.message}');
    }
  }

  // ── Edit ───────────────────────────────────────────────────────────

  Future<void> _showEditSheet() async {
    final item = _item;
    if (item == null) return;

    final result = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => _EditItemPage(api: widget.api, item: item),
      ),
    );
    if (result == true && mounted) {
      _modified = true;
      _loadItem();
    }
  }

  // ── Delete ─────────────────────────────────────────────────────────

  Future<void> _deleteItem() async {
    final confirm = await _confirmDialog(
      'Delete "${_item?.displayName ?? 'Item'}"?',
      'This item will be soft-deleted and can be restored later.',
    );
    if (confirm != true || !mounted) return;

    try {
      await widget.api.deleteItem(widget.itemId);
      if (!mounted) return;
      Navigator.pop(context, 'deleted');
    } on ApiError catch (e) {
      if (mounted) _snack('Delete failed: ${e.message}');
    }
  }

  // ── Move ───────────────────────────────────────────────────────────

  Future<void> _showMovePicker() async {
    final result = await showModalBottomSheet<_PickerResult>(
      context: context,
      isScrollControlled: true,
      builder: (ctx) => DraggableScrollableSheet(
        initialChildSize: 0.7,
        minChildSize: 0.4,
        maxChildSize: 0.9,
        expand: false,
        builder: (ctx, scrollController) => _ContainerPickerSheet(
          api: widget.api,
          scrollController: scrollController,
        ),
      ),
    );
    if (result == null || !mounted) return;

    try {
      await widget.api.moveItem(widget.itemId, result.id);
      if (!mounted) return;
      _snack('Moved to ${result.name}');
      _modified = true;
      _loadItem();
    } on ApiError catch (e) {
      if (mounted) _snack('Move failed: ${e.message}');
    }
  }

  // ── Barcodes ───────────────────────────────────────────────────────

  Future<void> _showBarcodeSheet() async {
    final item = _item;
    if (item == null) return;

    await showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      builder: (ctx) => DraggableScrollableSheet(
        initialChildSize: 0.6,
        minChildSize: 0.3,
        maxChildSize: 0.85,
        expand: false,
        builder: (ctx, scrollController) => _BarcodeSheet(
          api: widget.api,
          itemId: widget.itemId,
          systemBarcode: item.systemBarcode,
          externalCodes: item.externalCodes,
          scrollController: scrollController,
        ),
      ),
    );

    if (mounted) {
      _modified = true;
      _loadItem();
    }
  }

  // ── Helpers ────────────────────────────────────────────────────────

  Future<bool?> _confirmDialog(String title, String content) {
    return showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(title),
        content: Text(content),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('Confirm'),
          ),
        ],
      ),
    );
  }

  void _snack(String msg) {
    ScaffoldMessenger.of(context)
      ..hideCurrentSnackBar()
      ..showSnackBar(SnackBar(content: Text(msg)));
  }

  // ── Quantity adjustment (fungible items) ──────────────────────────

  Future<void> _showAdjustQuantityDialog() async {
    final item = _item!;
    final qtyCtrl = TextEditingController(
        text: item.fungibleQuantity?.toString() ?? '0');
    final reasonCtrl = TextEditingController();
    final result = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Adjust Quantity'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: qtyCtrl,
              decoration: InputDecoration(
                labelText: 'New quantity'
                    '${item.fungibleUnit != null ? ' (${item.fungibleUnit})' : ''}',
                border: const OutlineInputBorder(),
              ),
              keyboardType: TextInputType.number,
              autofocus: true,
            ),
            const SizedBox(height: 12),
            TextField(
              controller: reasonCtrl,
              decoration: const InputDecoration(
                labelText: 'Reason (optional)',
                border: OutlineInputBorder(),
              ),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('Save'),
          ),
        ],
      ),
    );
    final qtyText = qtyCtrl.text.trim();
    final reasonText = reasonCtrl.text.trim();
    qtyCtrl.dispose();
    reasonCtrl.dispose();
    if (result != true || !mounted) return;
    final qty = int.tryParse(qtyText);
    if (qty == null) return;
    try {
      await widget.api.adjustQuantity(widget.itemId, qty,
          reason: reasonText.isEmpty ? null : reasonText);
      if (!mounted) return;
      _modified = true;
      _loadItem();
    } on ApiError catch (e) {
      if (mounted) _snack('Failed: ${e.message}');
    }
  }

  // ── Build ─────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    return PopScope(
      canPop: false,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop) {
          Navigator.pop(context, _modified ? 'updated' : null);
        }
      },
      child: Scaffold(
        floatingActionButton: (_item?.isFungible == true)
            ? FloatingActionButton.extended(
                onPressed: _showAdjustQuantityDialog,
                icon: const Icon(Icons.edit),
                label: const Text('Qty'),
              )
            : null,
        appBar: AppBar(
          title: Text(_item?.displayName ?? 'Item'),
          actions: [
            IconButton(
              icon: const Icon(Icons.camera_alt_outlined),
              onPressed: _item != null ? _takePhoto : null,
              tooltip: 'Take photo',
            ),
            IconButton(
              icon: const Icon(Icons.refresh),
              onPressed: _loadItem,
              tooltip: 'Refresh',
            ),
            if (_item != null)
              PopupMenuButton<String>(
                onSelected: (action) {
                  switch (action) {
                    case 'edit':
                      _showEditSheet();
                    case 'move':
                      _showMovePicker();
                    case 'barcodes':
                      _showBarcodeSheet();
                    case 'delete':
                      _deleteItem();
                  }
                },
                itemBuilder: (_) => [
                  const PopupMenuItem(
                    value: 'edit',
                    child: Row(children: [
                      Icon(Icons.edit_outlined, size: 18),
                      SizedBox(width: 8),
                      Text('Edit'),
                    ]),
                  ),
                  const PopupMenuItem(
                    value: 'move',
                    child: Row(children: [
                      Icon(Icons.drive_file_move_outlined, size: 18),
                      SizedBox(width: 8),
                      Text('Move'),
                    ]),
                  ),
                  const PopupMenuItem(
                    value: 'barcodes',
                    child: Row(children: [
                      Icon(Icons.qr_code, size: 18),
                      SizedBox(width: 8),
                      Text('Barcodes'),
                    ]),
                  ),
                  const PopupMenuDivider(),
                  const PopupMenuItem(
                    value: 'delete',
                    child: Row(children: [
                      Icon(Icons.delete_outline, size: 18, color: Colors.red),
                      SizedBox(width: 8),
                      Text('Delete', style: TextStyle(color: Colors.red)),
                    ]),
                  ),
                ],
              ),
          ],
        ),
        body: _buildBody(),
      ),
    );
  }

  Widget _buildBody() {
    final theme = Theme.of(context);

    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }

    if (_error != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.error_outline,
                size: 48, color: theme.colorScheme.error),
            const SizedBox(height: 12),
            Text(_error!, style: TextStyle(color: theme.colorScheme.error)),
            const SizedBox(height: 16),
            FilledButton.tonal(
              onPressed: _loadItem,
              child: const Text('Retry'),
            ),
          ],
        ),
      );
    }

    final item = _item!;

    return Stack(
      children: [
        RefreshIndicator(
          onRefresh: _loadItem,
          child: ListView(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 80),
            children: [
              // ── Images or placeholder ──
              _buildImageSection(item, theme),

              // ── Name + condition badge ──
              const SizedBox(height: 16),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(
                    child: Text(
                      item.displayName,
                      style: theme.textTheme.headlineSmall
                          ?.copyWith(fontWeight: FontWeight.bold),
                    ),
                  ),
                  if (item.condition != null)
                    Container(
                      margin: const EdgeInsets.only(left: 8, top: 4),
                      padding: const EdgeInsets.symmetric(
                          horizontal: 8, vertical: 3),
                      decoration: BoxDecoration(
                        color: _conditionColor(item.condition!)
                            .withValues(alpha: 0.15),
                        borderRadius: BorderRadius.circular(12),
                        border: Border.all(
                          color: _conditionColor(item.condition!)
                              .withValues(alpha: 0.4),
                        ),
                      ),
                      child: Text(
                        _conditionLabel(item.condition!),
                        style: TextStyle(
                          fontSize: 12,
                          fontWeight: FontWeight.w600,
                          color: _conditionColor(item.condition!),
                        ),
                      ),
                    ),
                ],
              ),

              // ── System barcode ──
              if (item.systemBarcode != null) ...[
                const SizedBox(height: 4),
                Text(
                  item.systemBarcode!,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                    fontFamily: 'monospace',
                  ),
                ),
              ],

              // ── Location breadcrumb (tappable) ──
              if (item.ancestors.isNotEmpty) ...[
                const SizedBox(height: 12),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Padding(
                      padding: const EdgeInsets.only(top: 2),
                      child: Icon(Icons.place_outlined,
                          size: 16,
                          color: theme.colorScheme.onSurfaceVariant),
                    ),
                    const SizedBox(width: 6),
                    Expanded(
                      child: Wrap(
                        crossAxisAlignment: WrapCrossAlignment.center,
                        spacing: 2,
                        children: [
                          for (int i = 0;
                              i < item.ancestors.length;
                              i++) ...[
                            InkWell(
                              onTap: () => Navigator.push(
                                context,
                                MaterialPageRoute(
                                  builder: (_) => ItemDetailScreen(
                                    api: widget.api,
                                    itemId: item.ancestors[i].id,
                                  ),
                                ),
                              ),
                              borderRadius: BorderRadius.circular(4),
                              child: Padding(
                                padding: const EdgeInsets.symmetric(
                                    horizontal: 2, vertical: 2),
                                child: Text(
                                  item.ancestors[i].name ?? '?',
                                  style: theme.textTheme.bodySmall
                                      ?.copyWith(
                                    color: theme.colorScheme.primary,
                                    decoration:
                                        TextDecoration.underline,
                                  ),
                                ),
                              ),
                            ),
                            if (i < item.ancestors.length - 1)
                              Icon(Icons.chevron_right,
                                  size: 14,
                                  color: theme
                                      .colorScheme.onSurfaceVariant),
                          ],
                        ],
                      ),
                    ),
                  ],
                ),
              ],

              // ── Chips row: category, type, fungible ──
              const SizedBox(height: 12),
              Wrap(
                spacing: 8,
                runSpacing: 6,
                children: [
                  if (item.category != null)
                    _chip(Icons.category_outlined, item.category!, theme),
                  if (item.isContainer)
                    _chip(Icons.inventory_2_outlined, 'Container', theme),
                  if (item.isFungible)
                    _chip(Icons.grain, 'Fungible', theme),
                ],
              ),

              // ── Tags ──
              if (item.tags.isNotEmpty) ...[
                const SizedBox(height: 10),
                Wrap(
                  spacing: 6,
                  runSpacing: 4,
                  children: item.tags
                      .map((t) => Chip(
                            label:
                                Text(t, style: const TextStyle(fontSize: 12)),
                            visualDensity: VisualDensity.compact,
                            padding: EdgeInsets.zero,
                            materialTapTargetSize:
                                MaterialTapTargetSize.shrinkWrap,
                          ))
                      .toList(),
                ),
              ],

              // ── Description ──
              if (item.description != null &&
                  item.description!.isNotEmpty) ...[
                const SizedBox(height: 16),
                Text(item.description!, style: theme.textTheme.bodyMedium),
              ],

              // ── Properties card ──
              const SizedBox(height: 20),
              _buildPropertiesCard(item, theme),

              // ── Valuation card ──
              if (_hasValuation(item)) ...[
                const SizedBox(height: 12),
                _buildValuationCard(item, theme),
              ],

              // ── External codes ──
              if (item.externalCodes.isNotEmpty) ...[
                const SizedBox(height: 20),
                _sectionHeader('External Codes', Icons.qr_code, theme),
                const SizedBox(height: 8),
                ...item.externalCodes.map((c) => _buildCodeTile(c, theme)),
              ],

              // ── Metadata ──
              if (item.metadata.isNotEmpty) ...[
                const SizedBox(height: 20),
                _sectionHeader('Metadata', Icons.data_object, theme),
                const SizedBox(height: 8),
                _buildMetadataCard(item.metadata, theme),
              ],

              // ── Container contents ──
              if (item.isContainer) ...[
                const SizedBox(height: 24),
                _buildContainerContents(theme),
              ],

              // ── History (collapsible) ──
              const SizedBox(height: 20),
              _buildHistorySection(theme),
            ],
          ),
        ),

        // Lightbox overlay
        if (_lightboxIndex != null) _buildLightbox(item),
      ],
    );
  }

  // ── Image section ──────────────────────────────────────────────────

  Widget _buildImageSection(Item item, ThemeData theme) {
    if (item.images.isEmpty) {
      return GestureDetector(
        onTap: _takePhoto,
        child: Container(
          height: 160,
          decoration: BoxDecoration(
            color: theme.colorScheme.surfaceContainerHighest,
            borderRadius: BorderRadius.circular(12),
          ),
          child: Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(Icons.add_a_photo_outlined,
                    size: 40, color: theme.colorScheme.onSurfaceVariant),
                const SizedBox(height: 8),
                Text('Take a photo',
                    style: theme.textTheme.bodyMedium?.copyWith(
                      color: theme.colorScheme.onSurfaceVariant,
                    )),
              ],
            ),
          ),
        ),
      );
    }

    if (item.images.length == 1) {
      return GestureDetector(
        onTap: () => setState(() => _lightboxIndex = 0),
        child: ClipRRect(
          borderRadius: BorderRadius.circular(12),
          child: Container(
            color: theme.colorScheme.surfaceContainerHighest,
            child: Image.network(
              widget.api.imageUrl(item.images[0].path),
              fit: BoxFit.contain,
              height: 240,
              width: double.infinity,
            ),
          ),
        ),
      );
    }

    return SizedBox(
      height: 180,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        itemCount: item.images.length,
        separatorBuilder: (_, __) => const SizedBox(width: 8),
        itemBuilder: (_, i) => GestureDetector(
          onTap: () => setState(() => _lightboxIndex = i),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(8),
            child: Container(
              color: theme.colorScheme.surfaceContainerHighest,
              width: 180,
              child: Image.network(
                widget.api.imageUrl(item.images[i].path),
                fit: BoxFit.contain,
              ),
            ),
          ),
        ),
      ),
    );
  }

  // ── Properties card ────────────────────────────────────────────────

  Widget _buildPropertiesCard(Item item, ThemeData theme) {
    final rows = <_PropRow>[];

    rows.add(_PropRow('Type', item.isContainer ? 'Container' : 'Item'));

    if (item.isFungible && item.fungibleQuantity != null) {
      final qty = '${item.fungibleQuantity}'
          '${item.fungibleUnit != null ? ' ${item.fungibleUnit}' : ''}';
      rows.add(_PropRow('Quantity', qty));
    }

    if (item.systemBarcode != null) {
      rows.add(_PropRow('Barcode', item.systemBarcode!, mono: true));
    }

    if (item.category != null) {
      rows.add(_PropRow('Category', item.category!));
    }

    if (item.condition != null) {
      rows.add(_PropRow('Condition', _conditionLabel(item.condition!)));
    }

    if (item.weightGrams != null) {
      final w = item.weightGrams!;
      rows.add(_PropRow('Weight', w >= 1000
          ? '${(w / 1000).toStringAsFixed(2)} kg'
          : '${w.toStringAsFixed(0)} g'));
    }

    rows.add(_PropRow('Created', _formatDateTime(item.createdAt)));
    rows.add(_PropRow('Updated', _formatDateTime(item.updatedAt)));

    return Card(
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Column(
          children: rows.asMap().entries.map((e) {
            final row = e.value;
            return Column(
              children: [
                Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
                  child: Row(
                    children: [
                      SizedBox(
                        width: 100,
                        child: Text(row.label,
                            style: theme.textTheme.bodySmall?.copyWith(
                              color: theme.colorScheme.onSurfaceVariant,
                            )),
                      ),
                      Expanded(
                        child: Text(
                          row.value,
                          style: theme.textTheme.bodyMedium?.copyWith(
                            fontFamily: row.mono ? 'monospace' : null,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                if (e.key < rows.length - 1)
                  Divider(
                      height: 1,
                      indent: 16,
                      endIndent: 16,
                      color: theme.colorScheme.outlineVariant
                          .withValues(alpha: 0.4)),
              ],
            );
          }).toList(),
        ),
      ),
    );
  }

  // ── Valuation card ─────────────────────────────────────────────────

  bool _hasValuation(Item item) =>
      item.acquisitionDate != null ||
      item.acquisitionCost != null ||
      item.currentValue != null ||
      item.warrantyExpiry != null;

  Widget _buildValuationCard(Item item, ThemeData theme) {
    return Card(
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(Icons.payments_outlined,
                    size: 18, color: theme.colorScheme.onSurfaceVariant),
                const SizedBox(width: 8),
                Text('Valuation',
                    style: theme.textTheme.titleSmall
                        ?.copyWith(fontWeight: FontWeight.w600)),
              ],
            ),
            const SizedBox(height: 12),
            Wrap(
              spacing: 24,
              runSpacing: 12,
              children: [
                if (item.acquisitionDate != null)
                  _valuationField(
                      'Acquired', _formatDate(item.acquisitionDate!), theme),
                if (item.acquisitionCost != null)
                  _valuationField(
                      'Cost',
                      _formatCurrency(item.acquisitionCost!, item.currency),
                      theme),
                if (item.currentValue != null)
                  _valuationField(
                      'Value',
                      _formatCurrency(item.currentValue!, item.currency),
                      theme),
                if (item.warrantyExpiry != null)
                  _valuationField(
                      'Warranty', _formatDate(item.warrantyExpiry!), theme),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _valuationField(String label, String value, ThemeData theme) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label,
            style: theme.textTheme.bodySmall
                ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
        const SizedBox(height: 2),
        Text(value, style: theme.textTheme.bodyMedium),
      ],
    );
  }

  // ── External code tile ─────────────────────────────────────────────

  Widget _buildCodeTile(ExternalCodeEntry c, ThemeData theme) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        children: [
          Icon(Icons.qr_code, size: 16, color: theme.colorScheme.onSurfaceVariant),
          const SizedBox(width: 10),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
            decoration: BoxDecoration(
              color: theme.colorScheme.secondaryContainer,
              borderRadius: BorderRadius.circular(4),
            ),
            child: Text(c.codeType,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.onSecondaryContainer,
                )),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(c.value,
                style: theme.textTheme.bodyMedium
                    ?.copyWith(fontFamily: 'monospace', fontSize: 13)),
          ),
        ],
      ),
    );
  }

  // ── Metadata card ──────────────────────────────────────────────────

  Widget _buildMetadataCard(Map<String, dynamic> metadata, ThemeData theme) {
    return Card(
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          children: metadata.entries.map((e) {
            final val = e.value?.toString() ?? 'null';
            return Padding(
              padding: const EdgeInsets.symmetric(vertical: 4),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  SizedBox(
                    width: 120,
                    child: Text(e.key,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                          fontWeight: FontWeight.w600,
                        )),
                  ),
                  Expanded(
                    child: Text(val, style: theme.textTheme.bodySmall),
                  ),
                ],
              ),
            );
          }).toList(),
        ),
      ),
    );
  }

  // ── Container contents ─────────────────────────────────────────────

  Widget _buildContainerContents(ThemeData theme) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Icon(Icons.folder_open,
                size: 18, color: theme.colorScheme.onSurfaceVariant),
            const SizedBox(width: 8),
            Text('Contents',
                style: theme.textTheme.titleSmall
                    ?.copyWith(fontWeight: FontWeight.w600)),
            if (_children != null) ...[
              const SizedBox(width: 8),
              Container(
                padding:
                    const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                decoration: BoxDecoration(
                  color: theme.colorScheme.secondaryContainer,
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Text('${_children!.length}',
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: theme.colorScheme.onSecondaryContainer,
                    )),
              ),
            ],
          ],
        ),
        const Divider(),
        if (_loadingChildren)
          const Padding(
            padding: EdgeInsets.all(16),
            child: Center(child: CircularProgressIndicator()),
          )
        else if (_children != null && _children!.isEmpty)
          Padding(
            padding: const EdgeInsets.all(16),
            child: Center(
              child: Text('Empty container',
                  style: theme.textTheme.bodyMedium?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  )),
            ),
          )
        else if (_children != null)
          ..._children!.map((child) => ListTile(
                dense: true,
                contentPadding: EdgeInsets.zero,
                leading: Icon(
                  child.isContainer ? Icons.folder : Icons.inventory_2_outlined,
                  size: 20,
                  color: child.isContainer
                      ? theme.colorScheme.primary
                      : theme.colorScheme.onSurfaceVariant,
                ),
                title: Text(child.displayName,
                    style: const TextStyle(fontSize: 14)),
                subtitle: child.category != null
                    ? Text(child.category!,
                        style: theme.textTheme.bodySmall)
                    : null,
                trailing: const Icon(Icons.chevron_right, size: 16),
                onTap: () async {
                  final result = await Navigator.push<String>(
                    context,
                    MaterialPageRoute(
                      builder: (_) => ItemDetailScreen(
                        api: widget.api,
                        itemId: child.id,
                      ),
                    ),
                  );
                  if (mounted) {
                    if (result == 'deleted') _loadChildren();
                    _loadItem();
                  }
                },
              )),
        if (_hasMoreChildren)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 8),
            child: Center(
              child: _loadingMoreChildren
                  ? const SizedBox(
                      width: 24,
                      height: 24,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : TextButton.icon(
                      onPressed: _loadMoreChildren,
                      icon: const Icon(Icons.expand_more, size: 18),
                      label: const Text('Load more'),
                    ),
            ),
          ),
      ],
    );
  }

  // ── History section ─────────────────────────────────────────────────

  Widget _buildHistorySection(ThemeData theme) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        InkWell(
          onTap: () {
            setState(() => _historyExpanded = !_historyExpanded);
            if (_historyExpanded && _history == null) _loadHistory();
          },
          child: Row(
            children: [
              Icon(Icons.history,
                  size: 18, color: theme.colorScheme.onSurfaceVariant),
              const SizedBox(width: 8),
              Text('History',
                  style: theme.textTheme.titleSmall
                      ?.copyWith(fontWeight: FontWeight.w600)),
              const Spacer(),
              Icon(
                _historyExpanded ? Icons.expand_less : Icons.expand_more,
                size: 20,
              ),
            ],
          ),
        ),
        if (_historyExpanded) ...[
          const Divider(),
          if (_loadingHistory)
            const Center(
              child: Padding(
                padding: EdgeInsets.all(16),
                child: CircularProgressIndicator(),
              ),
            )
          else if (_history == null || _history!.isEmpty)
            Padding(
              padding: const EdgeInsets.all(16),
              child: Text('No history',
                  style: theme.textTheme.bodyMedium?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  )),
            )
          else
            ..._history!.map((e) => Padding(
                  padding: const EdgeInsets.symmetric(vertical: 6),
                  child: Row(
                    children: [
                      Expanded(
                        child: Text(
                          _formatEventType(e.eventType),
                          style: theme.textTheme.bodySmall,
                        ),
                      ),
                      Text(
                        _formatDateTime(e.createdAt),
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ),
                )),
        ],
      ],
    );
  }

  static String _formatEventType(String eventType) {
    // "ItemCreated" → "Item Created", "ItemQuantityAdjusted" → "Item Quantity Adjusted"
    return eventType
        .replaceAllMapped(RegExp(r'([A-Z])'), (m) => ' ${m[0]}')
        .trim();
  }

  // ── Helpers ────────────────────────────────────────────────────────

  Widget _sectionHeader(String title, IconData icon, ThemeData theme) {
    return Row(
      children: [
        Icon(icon, size: 18, color: theme.colorScheme.onSurfaceVariant),
        const SizedBox(width: 8),
        Text(title,
            style: theme.textTheme.titleSmall
                ?.copyWith(fontWeight: FontWeight.w600)),
      ],
    );
  }

  Widget _chip(IconData icon, String label, ThemeData theme) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
      decoration: BoxDecoration(
        color: theme.colorScheme.secondaryContainer.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(16),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(icon, size: 14, color: theme.colorScheme.onSecondaryContainer),
          const SizedBox(width: 6),
          Text(label,
              style: theme.textTheme.labelMedium?.copyWith(
                color: theme.colorScheme.onSecondaryContainer,
              )),
        ],
      ),
    );
  }

  Color _conditionColor(String condition) {
    return switch (condition) {
      'new' => Colors.green,
      'like_new' => Colors.teal,
      'good' => Colors.blue,
      'fair' => Colors.orange,
      'poor' => Colors.deepOrange,
      'broken' => Colors.red,
      _ => Colors.grey,
    };
  }

  // ── Lightbox ──────────────────────────────────────────────────────

  Widget _buildLightbox(Item item) {
    final idx = _lightboxIndex!;

    return GestureDetector(
      onTap: () => setState(() => _lightboxIndex = null),
      onHorizontalDragEnd: (details) {
        if (details.primaryVelocity == null) return;
        if (details.primaryVelocity! < -200 && idx < item.images.length - 1) {
          setState(() => _lightboxIndex = idx + 1);
        } else if (details.primaryVelocity! > 200 && idx > 0) {
          setState(() => _lightboxIndex = idx - 1);
        }
      },
      child: Container(
        color: Colors.black.withValues(alpha: 0.9),
        child: Stack(
          children: [
            Center(
              child: InteractiveViewer(
                child: Image.network(
                  widget.api.imageUrl(item.images[idx].path),
                  fit: BoxFit.contain,
                ),
              ),
            ),
            Positioned(
              top: 16,
              right: 16,
              child: IconButton(
                icon: const Icon(Icons.close, color: Colors.white, size: 28),
                onPressed: () => setState(() => _lightboxIndex = null),
              ),
            ),
            Positioned(
              top: 16,
              left: 16,
              child: IconButton(
                icon: const Icon(Icons.delete_outline,
                    color: Colors.red, size: 28),
                onPressed: () => _deleteImage(idx),
                tooltip: 'Delete image',
              ),
            ),
            if (item.images.length > 1)
              Positioned(
                top: 20,
                left: 0,
                right: 0,
                child: Center(
                  child: Container(
                    padding: const EdgeInsets.symmetric(
                        horizontal: 12, vertical: 4),
                    decoration: BoxDecoration(
                      color: Colors.black54,
                      borderRadius: BorderRadius.circular(12),
                    ),
                    child: Text(
                      '${idx + 1} / ${item.images.length}',
                      style:
                          const TextStyle(color: Colors.white, fontSize: 13),
                    ),
                  ),
                ),
              ),
            if (idx > 0)
              Positioned(
                left: 8,
                top: 0,
                bottom: 0,
                child: Center(
                  child: IconButton(
                    icon: const Icon(Icons.chevron_left,
                        color: Colors.white70, size: 36),
                    onPressed: () =>
                        setState(() => _lightboxIndex = idx - 1),
                  ),
                ),
              ),
            if (idx < item.images.length - 1)
              Positioned(
                right: 8,
                top: 0,
                bottom: 0,
                child: Center(
                  child: IconButton(
                    icon: const Icon(Icons.chevron_right,
                        color: Colors.white70, size: 36),
                    onPressed: () =>
                        setState(() => _lightboxIndex = idx + 1),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

// ── Properties row data ─────────────────────────────────────────────

class _PropRow {
  final String label;
  final String value;
  final bool mono;
  const _PropRow(this.label, this.value, {this.mono = false});
}

// ── Full-page Edit screen ───────────────────────────────────────────

class _EditItemPage extends StatefulWidget {
  final HomorgApi api;
  final Item item;

  const _EditItemPage({required this.api, required this.item});

  @override
  State<_EditItemPage> createState() => _EditItemPageState();
}

class _EditItemPageState extends State<_EditItemPage> {
  late final TextEditingController _nameCtrl;
  late final TextEditingController _descCtrl;
  late final TextEditingController _categoryCtrl;
  late String? _condition;
  late final List<String> _tags;
  final _tagCtrl = TextEditingController();
  late bool _isFungible;
  late final TextEditingController _quantityCtrl;
  late final TextEditingController _unitCtrl;

  // Valuation
  late final TextEditingController _acqDateCtrl;
  late final TextEditingController _acqCostCtrl;
  late final TextEditingController _valueCtrl;
  late final TextEditingController _warrantyCtrl;

  // Additional fields
  late bool _isContainer;
  late final TextEditingController _currencyCtrl;
  late final TextEditingController _weightCtrl;

  bool _saving = false;

  // Taxonomy
  List<String> _knownCategories = [];
  List<String> _knownTags = [];

  @override
  void initState() {
    super.initState();
    final item = widget.item;
    _nameCtrl = TextEditingController(text: item.name ?? '');
    _descCtrl = TextEditingController(text: item.description ?? '');
    _categoryCtrl = TextEditingController(text: item.category ?? '');
    _condition = item.condition;
    _tags = List.from(item.tags);
    _isFungible = item.isFungible;
    _quantityCtrl =
        TextEditingController(text: item.fungibleQuantity?.toString() ?? '');
    _unitCtrl = TextEditingController(text: item.fungibleUnit ?? '');

    _acqDateCtrl =
        TextEditingController(text: item.acquisitionDate ?? '');
    _acqCostCtrl =
        TextEditingController(text: item.acquisitionCost?.toString() ?? '');
    _valueCtrl =
        TextEditingController(text: item.currentValue?.toString() ?? '');
    _warrantyCtrl =
        TextEditingController(text: item.warrantyExpiry ?? '');

    _isContainer = item.isContainer;
    _currencyCtrl = TextEditingController(text: item.currency ?? '');
    _weightCtrl =
        TextEditingController(text: item.weightGrams?.toString() ?? '');

    _fetchTaxonomy();
  }

  Future<void> _fetchTaxonomy() async {
    try {
      final results = await Future.wait([
        widget.api.listCategories(),
        widget.api.listTags(),
      ]);
      if (mounted) {
        setState(() {
          _knownCategories = results[0];
          _knownTags = results[1];
        });
      }
    } catch (_) {}
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _descCtrl.dispose();
    _categoryCtrl.dispose();
    _tagCtrl.dispose();
    _quantityCtrl.dispose();
    _unitCtrl.dispose();
    _acqDateCtrl.dispose();
    _acqCostCtrl.dispose();
    _valueCtrl.dispose();
    _warrantyCtrl.dispose();
    _currencyCtrl.dispose();
    _weightCtrl.dispose();
    super.dispose();
  }

  void _addTag() {
    final tag = _tagCtrl.text.trim();
    if (tag.isNotEmpty && !_tags.contains(tag)) {
      setState(() => _tags.add(tag));
      _tagCtrl.clear();
    }
  }

  Future<void> _pickDate(TextEditingController ctrl) async {
    final initial = DateTime.tryParse(ctrl.text) ?? DateTime.now();
    final picked = await showDatePicker(
      context: context,
      initialDate: initial,
      firstDate: DateTime(2000),
      lastDate: DateTime(2100),
    );
    if (picked != null) {
      ctrl.text =
          '${picked.year}-${picked.month.toString().padLeft(2, '0')}-${picked.day.toString().padLeft(2, '0')}';
    }
  }

  Future<void> _save() async {
    final item = widget.item;
    final body = <String, dynamic>{};

    // Text fields — send null to clear, omit if unchanged
    void diffText(String key, String newVal, String? oldVal) {
      final trimmed = newVal.trim();
      final old = oldVal ?? '';
      if (trimmed != old) {
        body[key] = trimmed.isEmpty ? null : trimmed;
      }
    }

    diffText('name', _nameCtrl.text, item.name);
    diffText('description', _descCtrl.text, item.description);
    diffText('category', _categoryCtrl.text, item.category);

    if (_condition != item.condition) {
      body['condition'] = _condition;
    }

    final sortedOld = List<String>.from(item.tags)..sort();
    final sortedNew = List<String>.from(_tags)..sort();
    if (sortedOld.join(',') != sortedNew.join(',')) {
      body['tags'] = _tags;
    }

    // Fungible
    if (_isFungible != item.isFungible) {
      body['is_fungible'] = _isFungible;
    }
    if (_isFungible) {
      final qty = int.tryParse(_quantityCtrl.text.trim());
      if (qty != item.fungibleQuantity) {
        body['fungible_quantity'] = qty;
      }
      final unit = _unitCtrl.text.trim();
      if (unit != (item.fungibleUnit ?? '')) {
        body['fungible_unit'] = unit.isEmpty ? null : unit;
      }
    }

    // Valuation
    void diffDate(String key, String newVal, String? oldVal) {
      final trimmed = newVal.trim();
      final old = oldVal ?? '';
      if (trimmed != old) {
        body[key] = trimmed.isEmpty ? null : trimmed;
      }
    }

    void diffNum(String key, String newVal, double? oldVal) {
      final trimmed = newVal.trim();
      final newNum = double.tryParse(trimmed);
      if (trimmed.isEmpty && oldVal != null) {
        body[key] = null;
      } else if (newNum != null && newNum != oldVal) {
        body[key] = newNum;
      }
    }

    diffDate('acquisition_date', _acqDateCtrl.text, item.acquisitionDate);
    diffNum('acquisition_cost', _acqCostCtrl.text, item.acquisitionCost);
    diffNum('current_value', _valueCtrl.text, item.currentValue);
    diffDate('warranty_expiry', _warrantyCtrl.text, item.warrantyExpiry);

    // Additional fields
    if (_isContainer != item.isContainer) {
      body['is_container'] = _isContainer;
    }
    diffText('currency', _currencyCtrl.text, item.currency);
    final newWeight = double.tryParse(_weightCtrl.text.trim());
    final oldWeight = item.weightGrams;
    if (_weightCtrl.text.trim().isEmpty && oldWeight != null) {
      body['weight_grams'] = null;
    } else if (newWeight != null && newWeight != oldWeight) {
      body['weight_grams'] = newWeight;
    }

    if (body.isEmpty) {
      Navigator.pop(context, false);
      return;
    }

    setState(() => _saving = true);

    try {
      await widget.api.updateItem(item.id, body);
      if (!mounted) return;
      Navigator.pop(context, true);
    } on ApiError catch (e) {
      if (!mounted) return;
      setState(() => _saving = false);
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(e.message)));
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(
        title: const Text('Edit Item'),
        actions: [
          TextButton(
            onPressed: _saving ? null : _save,
            child: _saving
                ? const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Text('Save'),
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          // Name
          TextField(
            controller: _nameCtrl,
            decoration: const InputDecoration(
              labelText: 'Name',
              border: OutlineInputBorder(),
            ),
            textCapitalization: TextCapitalization.sentences,
          ),
          const SizedBox(height: 16),

          // Description
          TextField(
            controller: _descCtrl,
            decoration: const InputDecoration(
              labelText: 'Description',
              border: OutlineInputBorder(),
            ),
            maxLines: 4,
            minLines: 2,
            textCapitalization: TextCapitalization.sentences,
          ),
          const SizedBox(height: 16),

          // Category (autocomplete from known categories)
          Autocomplete<String>(
            initialValue: TextEditingValue(text: widget.item.category ?? ''),
            optionsBuilder: (textEditingValue) {
              if (_knownCategories.isEmpty) return const Iterable.empty();
              if (textEditingValue.text.isEmpty) return _knownCategories;
              final q = textEditingValue.text.toLowerCase();
              return _knownCategories
                  .where((c) => c.toLowerCase().contains(q));
            },
            onSelected: (value) => _categoryCtrl.text = value,
            fieldViewBuilder: (ctx, ctrl, fn, onSubmit) {
              // Keep _categoryCtrl in sync
              ctrl.addListener(() => _categoryCtrl.text = ctrl.text);
              return TextField(
                controller: ctrl,
                focusNode: fn,
                decoration: const InputDecoration(
                  labelText: 'Category',
                  border: OutlineInputBorder(),
                ),
                textCapitalization: TextCapitalization.sentences,
              );
            },
          ),
          const SizedBox(height: 16),

          // Condition
          DropdownButtonFormField<String?>(
            value: _condition,
            decoration: const InputDecoration(
              labelText: 'Condition',
              border: OutlineInputBorder(),
            ),
            items: [
              const DropdownMenuItem(value: null, child: Text('None')),
              ..._conditions.map((c) => DropdownMenuItem(
                    value: c,
                    child: Text(_conditionLabel(c)),
                  )),
            ],
            onChanged: (v) => setState(() => _condition = v),
          ),
          const SizedBox(height: 16),

          // Tags
          Text('Tags', style: theme.textTheme.labelLarge),
          const SizedBox(height: 8),
          if (_tags.isNotEmpty)
            Wrap(
              spacing: 6,
              runSpacing: 4,
              children: _tags
                  .map((t) => Chip(
                        label: Text(t, style: const TextStyle(fontSize: 12)),
                        onDeleted: () => setState(() => _tags.remove(t)),
                        visualDensity: VisualDensity.compact,
                      ))
                  .toList(),
            ),
          if (_tags.isNotEmpty) const SizedBox(height: 8),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _tagCtrl,
                  decoration: const InputDecoration(
                    hintText: 'Add tag',
                    isDense: true,
                    border: OutlineInputBorder(),
                  ),
                  onSubmitted: (_) => _addTag(),
                ),
              ),
              const SizedBox(width: 8),
              IconButton.filled(
                onPressed: _addTag,
                icon: const Icon(Icons.add, size: 20),
              ),
            ],
          ),
          // Tag suggestions from taxonomy
          if (_knownTags.where((t) => !_tags.contains(t)).isNotEmpty) ...[
            const SizedBox(height: 8),
            Wrap(
              spacing: 6,
              runSpacing: 4,
              children: _knownTags
                  .where((t) => !_tags.contains(t))
                  .map((t) => ActionChip(
                        label: Text(t, style: const TextStyle(fontSize: 12)),
                        visualDensity: VisualDensity.compact,
                        onPressed: () => setState(() => _tags.add(t)),
                      ))
                  .toList(),
            ),
          ],
          const SizedBox(height: 20),

          // Fungible toggle
          SwitchListTile(
            title: const Text('Fungible / consumable'),
            contentPadding: EdgeInsets.zero,
            value: _isFungible,
            onChanged: (v) => setState(() => _isFungible = v),
          ),
          if (_isFungible) ...[
            const SizedBox(height: 4),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _quantityCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Quantity',
                      border: OutlineInputBorder(),
                    ),
                    keyboardType: TextInputType.number,
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: TextField(
                    controller: _unitCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Unit',
                      hintText: 'e.g. ml, pcs',
                      border: OutlineInputBorder(),
                    ),
                  ),
                ),
              ],
            ),
          ],

          // ── Valuation section ──
          const SizedBox(height: 24),
          Text('Valuation', style: theme.textTheme.titleSmall),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _acqDateCtrl,
                  decoration: InputDecoration(
                    labelText: 'Acquisition date',
                    border: const OutlineInputBorder(),
                    suffixIcon: IconButton(
                      icon: const Icon(Icons.calendar_today, size: 18),
                      onPressed: () => _pickDate(_acqDateCtrl),
                    ),
                  ),
                  readOnly: true,
                  onTap: () => _pickDate(_acqDateCtrl),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: TextField(
                  controller: _acqCostCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Cost',
                    border: OutlineInputBorder(),
                  ),
                  keyboardType:
                      const TextInputType.numberWithOptions(decimal: true),
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _valueCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Current value',
                    border: OutlineInputBorder(),
                  ),
                  keyboardType:
                      const TextInputType.numberWithOptions(decimal: true),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: TextField(
                  controller: _warrantyCtrl,
                  decoration: InputDecoration(
                    labelText: 'Warranty expiry',
                    border: const OutlineInputBorder(),
                    suffixIcon: IconButton(
                      icon: const Icon(Icons.calendar_today, size: 18),
                      onPressed: () => _pickDate(_warrantyCtrl),
                    ),
                  ),
                  readOnly: true,
                  onTap: () => _pickDate(_warrantyCtrl),
                ),
              ),
            ],
          ),

          // ── Additional fields ──
          const SizedBox(height: 24),
          Text('Properties', style: theme.textTheme.titleSmall),
          const SizedBox(height: 12),
          SwitchListTile(
            title: const Text('Is a container'),
            subtitle: const Text('Can hold other items'),
            value: _isContainer,
            onChanged: (v) => setState(() => _isContainer = v),
            contentPadding: EdgeInsets.zero,
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _currencyCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Currency',
                    hintText: 'USD',
                    border: OutlineInputBorder(),
                  ),
                  textCapitalization: TextCapitalization.characters,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: TextField(
                  controller: _weightCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Weight (g)',
                    border: OutlineInputBorder(),
                  ),
                  keyboardType:
                      const TextInputType.numberWithOptions(decimal: true),
                ),
              ),
            ],
          ),

          const SizedBox(height: 80),
        ],
      ),
    );
  }
}

// ── Container picker (reused pattern) ───────────────────────────────

class _ContainerPickerSheet extends StatefulWidget {
  final HomorgApi api;
  final ScrollController scrollController;

  const _ContainerPickerSheet({
    required this.api,
    required this.scrollController,
  });

  @override
  State<_ContainerPickerSheet> createState() => _ContainerPickerSheetState();
}

class _ContainerPickerSheetState extends State<_ContainerPickerSheet> {
  final _searchController = TextEditingController();
  Timer? _debounce;
  List<ItemSummary>? _results;
  bool _searching = false;

  @override
  void initState() {
    super.initState();
    _doSearch('');
  }

  @override
  void dispose() {
    _searchController.dispose();
    _debounce?.cancel();
    super.dispose();
  }

  void _onSearchChanged(String query) {
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 300), () {
      _doSearch(query);
    });
  }

  Future<void> _doSearch(String query) async {
    setState(() => _searching = true);
    try {
      final results = await widget.api.searchContainers(query);
      if (mounted) {
        setState(() {
          _results = results;
          _searching = false;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _searching = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.only(top: 8, bottom: 4),
          child: Container(
            width: 40,
            height: 4,
            decoration: BoxDecoration(
              color: theme.colorScheme.onSurface.withValues(alpha: 0.3),
              borderRadius: BorderRadius.circular(2),
            ),
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
          child: TextField(
            controller: _searchController,
            decoration: const InputDecoration(
              hintText: 'Search containers…',
              prefixIcon: Icon(Icons.search),
              border: OutlineInputBorder(),
              isDense: true,
            ),
            autofocus: true,
            onChanged: _onSearchChanged,
          ),
        ),
        if (_searching) const LinearProgressIndicator(),
        Expanded(
          child: _results == null || _results!.isEmpty
              ? Center(
                  child: Text(
                    _results == null ? '' : 'No containers found',
                    style: theme.textTheme.bodyMedium?.copyWith(
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
                )
              : ListView.builder(
                  controller: widget.scrollController,
                  itemCount: _results!.length,
                  itemBuilder: (_, i) {
                    final c = _results![i];
                    return ListTile(
                      leading: const Icon(Icons.folder, size: 20),
                      title: Text(c.displayName),
                      subtitle: c.parentName != null
                          ? Text('in ${c.parentName}',
                              style: theme.textTheme.bodySmall)
                          : null,
                      dense: true,
                      onTap: () => Navigator.pop(
                        context,
                        _PickerResult(id: c.id, name: c.displayName),
                      ),
                    );
                  },
                ),
        ),
      ],
    );
  }
}

class _PickerResult {
  final String id;
  final String name;
  const _PickerResult({required this.id, required this.name});
}

// ── Barcode management sheet ────────────────────────────────────────

class _BarcodeSheet extends StatefulWidget {
  final HomorgApi api;
  final String itemId;
  final String? systemBarcode;
  final List<ExternalCodeEntry> externalCodes;
  final ScrollController scrollController;

  const _BarcodeSheet({
    required this.api,
    required this.itemId,
    required this.systemBarcode,
    required this.externalCodes,
    required this.scrollController,
  });

  @override
  State<_BarcodeSheet> createState() => _BarcodeSheetState();
}

class _BarcodeSheetState extends State<_BarcodeSheet> {
  late String? _systemBarcode;
  late List<ExternalCodeEntry> _codes;
  final _barcodeCtrl = TextEditingController();
  final _codeTypeCtrl = TextEditingController();
  final _codeValueCtrl = TextEditingController();
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    _systemBarcode = widget.systemBarcode;
    _codes = List.from(widget.externalCodes);
  }

  @override
  void dispose() {
    _barcodeCtrl.dispose();
    _codeTypeCtrl.dispose();
    _codeValueCtrl.dispose();
    super.dispose();
  }

  Future<void> _generateAndAssign() async {
    setState(() => _busy = true);
    try {
      final barcode = await widget.api.generateBarcode();
      await widget.api.assignBarcode(widget.itemId, barcode);
      if (mounted) {
        setState(() {
          _systemBarcode = barcode;
          _busy = false;
        });
      }
    } on ApiError catch (e) {
      if (mounted) {
        setState(() => _busy = false);
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(e.message)));
      }
    }
  }

  Future<void> _assignManual() async {
    final barcode = _barcodeCtrl.text.trim();
    if (barcode.isEmpty) return;
    setState(() => _busy = true);
    try {
      await widget.api.assignBarcode(widget.itemId, barcode);
      if (mounted) {
        setState(() {
          _systemBarcode = barcode;
          _busy = false;
        });
        _barcodeCtrl.clear();
      }
    } on ApiError catch (e) {
      if (mounted) {
        setState(() => _busy = false);
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(e.message)));
      }
    }
  }

  Future<void> _addExternalCode() async {
    final type = _codeTypeCtrl.text.trim();
    final value = _codeValueCtrl.text.trim();
    if (type.isEmpty || value.isEmpty) return;
    setState(() => _busy = true);
    try {
      await widget.api.addExternalCode(widget.itemId, type, value);
      if (mounted) {
        setState(() {
          _codes.add(ExternalCodeEntry(codeType: type, value: value));
          _busy = false;
        });
        _codeTypeCtrl.clear();
        _codeValueCtrl.clear();
      }
    } on ApiError catch (e) {
      if (mounted) {
        setState(() => _busy = false);
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(e.message)));
      }
    }
  }

  Future<void> _removeExternalCode(ExternalCodeEntry code) async {
    setState(() => _busy = true);
    try {
      await widget.api.removeExternalCode(
          widget.itemId, code.codeType, code.value);
      if (mounted) {
        setState(() {
          _codes.remove(code);
          _busy = false;
        });
      }
    } on ApiError catch (e) {
      if (mounted) {
        setState(() => _busy = false);
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(e.message)));
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.only(top: 8, bottom: 4),
          child: Container(
            width: 40,
            height: 4,
            decoration: BoxDecoration(
              color: theme.colorScheme.onSurface.withValues(alpha: 0.3),
              borderRadius: BorderRadius.circular(2),
            ),
          ),
        ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          child: Text('Barcodes', style: theme.textTheme.titleMedium),
        ),
        const Divider(height: 1),
        if (_busy) const LinearProgressIndicator(),
        Expanded(
          child: ListView(
            controller: widget.scrollController,
            padding: const EdgeInsets.all(16),
            children: [
              // System barcode section
              Text('System Barcode', style: theme.textTheme.labelLarge),
              const SizedBox(height: 8),
              if (_systemBarcode != null)
                Card(
                  child: Padding(
                    padding: const EdgeInsets.all(12),
                    child: Row(
                      children: [
                        const Icon(Icons.qr_code_2, size: 20),
                        const SizedBox(width: 8),
                        Text(_systemBarcode!,
                            style: const TextStyle(
                                fontFamily: 'monospace', fontSize: 14)),
                      ],
                    ),
                  ),
                )
              else
                Text('No system barcode assigned',
                    style: theme.textTheme.bodySmall),
              const SizedBox(height: 8),
              Row(
                children: [
                  OutlinedButton.icon(
                    onPressed: _busy ? null : _generateAndAssign,
                    icon: const Icon(Icons.auto_awesome, size: 16),
                    label: const Text('Generate'),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: TextField(
                      controller: _barcodeCtrl,
                      decoration: const InputDecoration(
                        hintText: 'Or enter manually',
                        isDense: true,
                        border: OutlineInputBorder(),
                      ),
                      onSubmitted: (_) => _assignManual(),
                    ),
                  ),
                  const SizedBox(width: 4),
                  IconButton(
                    onPressed: _busy ? null : _assignManual,
                    icon: const Icon(Icons.check),
                  ),
                ],
              ),
              const SizedBox(height: 24),
              // External codes section
              Text('External Codes', style: theme.textTheme.labelLarge),
              const SizedBox(height: 8),
              if (_codes.isEmpty)
                Text('No external codes', style: theme.textTheme.bodySmall),
              ..._codes.map((c) => ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    leading: const Icon(Icons.qr_code, size: 18),
                    title: Text(c.value,
                        style: const TextStyle(
                            fontSize: 13, fontFamily: 'monospace')),
                    subtitle: Text(c.codeType,
                        style: const TextStyle(fontSize: 11)),
                    trailing: IconButton(
                      icon: const Icon(Icons.close, size: 16),
                      onPressed:
                          _busy ? null : () => _removeExternalCode(c),
                    ),
                  )),
              const SizedBox(height: 8),
              Row(
                children: [
                  SizedBox(
                    width: 80,
                    child: TextField(
                      controller: _codeTypeCtrl,
                      decoration: const InputDecoration(
                        hintText: 'Type',
                        isDense: true,
                        border: OutlineInputBorder(),
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: TextField(
                      controller: _codeValueCtrl,
                      decoration: const InputDecoration(
                        hintText: 'Value',
                        isDense: true,
                        border: OutlineInputBorder(),
                      ),
                      onSubmitted: (_) => _addExternalCode(),
                    ),
                  ),
                  const SizedBox(width: 4),
                  IconButton(
                    onPressed: _busy ? null : _addExternalCode,
                    icon: const Icon(Icons.add),
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}
