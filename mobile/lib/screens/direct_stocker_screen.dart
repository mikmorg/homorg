import 'dart:async';
import 'dart:io';

import 'package:camera/camera.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bluetooth_serial/flutter_bluetooth_serial.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../models/item.dart';
import '../models/session.dart';
import '../services/homorg_api.dart';
import '../services/bluetooth_scanner_service.dart';
import 'camera_capture_screen.dart';
import 'item_detail_screen.dart';

enum _InputMode { camera, bluetooth }

const _btAddressPrefKey = 'last_bt_address';
const _btNamePrefKey = 'last_bt_name';

/// Full stocker session driven directly from the mobile app.
/// Scan/pick a container as context, then scan items to move or create them.
class DirectStockerScreen extends StatefulWidget {
  final HomorgApi api;
  final ScanSession session;

  const DirectStockerScreen({
    super.key,
    required this.api,
    required this.session,
  });

  @override
  State<DirectStockerScreen> createState() => _DirectStockerScreenState();
}

class _DirectStockerScreenState extends State<DirectStockerScreen> {
  late ScanSession _session;

  // Active context
  String? _activeContainerId;
  String? _activeContainerName;

  // Scanner
  final _scanner = BluetoothScannerService();
  StreamSubscription<String>? _scanSub;
  StreamSubscription<BtScannerState>? _scanStateSub;
  BtScannerState _scannerState = BtScannerState.disconnected;
  String? _savedBtAddress;
  String? _savedBtName;
  _InputMode _mode = _InputMode.camera;
  MobileScannerController? _cameraController;

  // Scan processing
  bool _processing = false;
  final List<_LogEntry> _log = [];

  @override
  void initState() {
    super.initState();
    _session = widget.session;
    _activeContainerId = _session.activeContainerId;

    _scanSub = _scanner.scans.listen(_onBarcode);
    _scanStateSub = _scanner.stateStream.listen((s) {
      if (mounted) setState(() => _scannerState = s);
    });
    _loadSavedBtDevice();
    _startCamera();

    // If session already has a context, fetch the container name
    if (_activeContainerId != null) {
      _fetchContainerName(_activeContainerId!);
    }
  }

  @override
  void dispose() {
    _scanSub?.cancel();
    _scanStateSub?.cancel();
    _scanner.dispose();
    _cameraController?.dispose();
    super.dispose();
  }

  // ── Camera lifecycle ──────────────────────────────────────────────

  void _startCamera() {
    _cameraController = MobileScannerController(
      detectionSpeed: DetectionSpeed.noDuplicates,
    );
  }

  void _stopCamera() {
    _cameraController?.dispose();
    _cameraController = null;
  }

  void _setMode(_InputMode mode) {
    if (mode == _mode) return;
    setState(() => _mode = mode);
    if (mode == _InputMode.camera) {
      _startCamera();
    } else {
      _stopCamera();
    }
  }

  void _onCameraDetect(BarcodeCapture capture) {
    for (final code in capture.barcodes) {
      final value = code.rawValue;
      if (value != null && value.isNotEmpty) {
        _onBarcode(value);
        return;
      }
    }
  }

  // ── BT scanner ────────────────────────────────────────────────────

  Future<void> _loadSavedBtDevice() async {
    final prefs = await SharedPreferences.getInstance();
    if (!mounted) return;
    final addr = prefs.getString(_btAddressPrefKey);
    final name = prefs.getString(_btNamePrefKey);
    setState(() {
      _savedBtAddress = addr;
      _savedBtName = name;
    });
    if (addr != null) _setMode(_InputMode.bluetooth);
  }

  Future<void> _saveBtDevice(BluetoothDevice device) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_btAddressPrefKey, device.address);
    if (device.name != null) {
      await prefs.setString(_btNamePrefKey, device.name!);
    }
    if (!mounted) return;
    setState(() {
      _savedBtAddress = device.address;
      _savedBtName = device.name;
    });
  }

  Future<void> _connectScanner() async {
    final granted = await BluetoothScannerService.ensurePermissions();
    if (!mounted) return;
    if (!granted) {
      _snack('Bluetooth permissions denied');
      return;
    }
    if (_savedBtAddress != null) {
      final device = BluetoothDevice(
        address: _savedBtAddress!,
        name: _savedBtName,
      );
      try {
        await _scanner.connect(device);
      } catch (e) {
        if (mounted) _snack('Connect failed: $e');
      }
      return;
    }
    await _showDevicePicker();
  }

  Future<void> _changeScanner() async {
    final granted = await BluetoothScannerService.ensurePermissions();
    if (!mounted) return;
    if (!granted) {
      _snack('Bluetooth permissions denied');
      return;
    }
    if (_scanner.isConnected) await _scanner.disconnect();
    await _showDevicePicker();
  }

  Future<void> _showDevicePicker() async {
    List<BluetoothDevice> devices;
    try {
      devices = await _scanner.bondedDevices();
    } catch (e) {
      if (mounted) _snack('Bluetooth unavailable: $e');
      return;
    }
    if (!mounted) return;
    if (devices.isEmpty) {
      _snack('No paired devices — pair your scanner in Android settings first');
      return;
    }
    final selected = await showModalBottomSheet<BluetoothDevice>(
      context: context,
      builder: (ctx) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          children: [
            const ListTile(
              title: Text('Paired devices',
                  style: TextStyle(fontWeight: FontWeight.bold)),
              dense: true,
            ),
            for (final d in devices)
              ListTile(
                leading: const Icon(Icons.bluetooth),
                title: Text(d.name ?? '(unnamed)'),
                subtitle: Text(d.address),
                trailing: d.address == _savedBtAddress
                    ? const Icon(Icons.check, size: 18)
                    : null,
                onTap: () => Navigator.of(ctx).pop(d),
              ),
          ],
        ),
      ),
    );
    if (selected == null || !mounted) return;
    await _saveBtDevice(selected);
    try {
      await _scanner.connect(selected);
    } catch (e) {
      if (mounted) _snack('Connect failed: $e');
    }
  }

  // ── Container management ──────────────────────────────────────────

  Future<void> _fetchContainerName(String id) async {
    try {
      final item = await widget.api.getItem(id);
      if (!mounted) return;
      setState(() => _activeContainerName = item.displayName);
    } catch (_) {
      // Non-critical — just show ID
    }
  }

  Future<void> _setContext(String containerId, String containerName) async {
    try {
      await widget.api.submitBatch(
        _session.id,
        [SetContextEvent(containerId: containerId)],
      );
      if (!mounted) return;
      setState(() {
        _activeContainerId = containerId;
        _activeContainerName = containerName;
      });
      _addLog(Icons.folder_open, 'Context set to $containerName',
          itemId: containerId);
    } on ApiError catch (e) {
      if (mounted) _snack('Failed to set context: ${e.message}');
    }
  }

  Future<void> _showContainerPicker() async {
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
    if (result != null && mounted) {
      _setContext(result.id, result.name);
    }
  }

  // ── Barcode processing ────────────────────────────────────────────

  Future<void> _onBarcode(String barcode) async {
    if (_processing) return;
    setState(() => _processing = true);

    try {
      final resolution = await widget.api.resolveBarcode(barcode);
      if (!mounted) return;
      await _handleResolution(barcode, resolution);
    } on ApiError catch (e) {
      if (!mounted) return;
      _addLog(Icons.error_outline, 'Error resolving $barcode: ${e.message}',
          isError: true);
    } finally {
      if (mounted) setState(() => _processing = false);
    }
  }

  Future<void> _handleResolution(
      String barcode, BarcodeResolution resolution) async {
    switch (resolution) {
      case SystemBarcode(:final itemId):
        // Existing item → check if container
        try {
          final item = await widget.api.getItem(itemId);
          if (!mounted) return;
          if (item.isContainer) {
            // Scanned a container → set as context
            _setContext(itemId, item.displayName);
          } else if (_activeContainerId == null) {
            _addLog(Icons.warning_amber, 'Set a container first before scanning items',
                isError: true);
          } else {
            // Move item into active container
            await widget.api.submitBatch(
              _session.id,
              [MoveItemEvent(itemId: itemId)],
            );
            if (!mounted) return;
            _addLog(Icons.move_to_inbox,
                'Moved "${item.displayName}" → $_activeContainerName',
                itemId: itemId);
            _refreshSession();
          }
        } on ApiError catch (e) {
          if (mounted) {
            _addLog(Icons.error_outline, 'Error: ${e.message}', isError: true);
          }
        }

      case Preset(:final barcode, :final isContainer, :final containerTypeName):
        if (isContainer) {
          if (_activeContainerId == null) {
            _addLog(Icons.warning_amber,
                'Set a parent container first before creating containers',
                isError: true);
            return;
          }
          // Create container preset and set as new context
          final resp = await widget.api.submitBatch(
            _session.id,
            [
              CreateAndPlaceEvent(
                barcode: barcode,
                isContainer: true,
              )
            ],
          );
          if (!mounted) return;
          if (resp.hasErrors) {
            _addLog(Icons.error_outline,
                'Error: ${resp.errors.first.message}',
                isError: true);
          } else {
            final newId = resp.results
                .whereType<BatchResult>()
                .where((r) => r.itemId != null)
                .firstOrNull
                ?.itemId;
            final label = containerTypeName ?? 'Container';
            _addLog(Icons.create_new_folder, 'Created $label "$barcode"',
                itemId: newId);
            if (newId != null) {
              _setContext(newId, barcode);
            }
            _refreshSession();
          }
        } else {
          // Item preset → create and place
          if (_activeContainerId == null) {
            _addLog(Icons.warning_amber, 'Set a container first',
                isError: true);
            return;
          }
          final resp = await widget.api.submitBatch(
            _session.id,
            [CreateAndPlaceEvent(barcode: barcode)],
          );
          if (!mounted) return;
          if (resp.hasErrors) {
            _addLog(Icons.error_outline,
                'Error: ${resp.errors.first.message}',
                isError: true);
          } else {
            final createdId = resp.results
                .whereType<BatchResult>()
                .where((r) => r.itemId != null)
                .firstOrNull
                ?.itemId;
            _addLog(Icons.add_circle_outline,
                'Created item "$barcode" in $_activeContainerName',
                itemId: createdId);
            _refreshSession();
          }
        }

      case UnknownSystem(:final barcode):
        // Unassigned system barcode → create and place
        if (_activeContainerId == null) {
          _addLog(Icons.warning_amber, 'Set a container first', isError: true);
          return;
        }
        final resp = await widget.api.submitBatch(
          _session.id,
          [CreateAndPlaceEvent(barcode: barcode)],
        );
        if (!mounted) return;
        if (resp.hasErrors) {
          _addLog(Icons.error_outline,
              'Error: ${resp.errors.first.message}',
              isError: true);
        } else {
          final createdId = resp.results
              .whereType<BatchResult>()
              .where((r) => r.itemId != null)
              .firstOrNull
              ?.itemId;
          _addLog(Icons.add_circle_outline,
              'Created item "$barcode" in $_activeContainerName',
              itemId: createdId);
          _refreshSession();
        }

      case ExternalCode(:final codeType, :final value, :final itemIds):
        if (itemIds.length == 1) {
          // Known external code → treat as system barcode
          _handleResolution(barcode,
              SystemBarcode(barcode: value, itemId: itemIds[0]));
        } else if (itemIds.length > 1) {
          _pickItemFromExternal(codeType, value, itemIds);
        } else {
          _addLog(Icons.qr_code, '$codeType: $value — not linked');
        }

      case Unknown(:final value):
        _addLog(Icons.help_outline, 'Unknown code: $value');
        _snack('Unknown code: $value');
    }
  }

  Future<void> _pickItemFromExternal(
      String codeType, String value, List<String> itemIds) async {
    List<Item> items;
    try {
      items = await Future.wait(itemIds.map(widget.api.getItem));
    } catch (_) {
      _addLog(Icons.error_outline, 'Failed to load items for $value',
          isError: true);
      return;
    }
    if (!mounted) return;

    final picked = await showModalBottomSheet<Item>(
      context: context,
      isScrollControlled: true,
      builder: (ctx) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              title: Text('${itemIds.length} items match $codeType: $value',
                  style: const TextStyle(fontWeight: FontWeight.bold)),
              dense: true,
            ),
            ...items.map((item) => ListTile(
                  leading: Icon(item.isContainer
                      ? Icons.folder
                      : Icons.inventory_2_outlined),
                  title: Text(item.displayName),
                  subtitle: item.locationBreadcrumb.isNotEmpty
                      ? Text(item.locationBreadcrumb,
                          maxLines: 1, overflow: TextOverflow.ellipsis)
                      : null,
                  onTap: () => Navigator.pop(ctx, item),
                )),
          ],
        ),
      ),
    );
    if (picked != null && mounted) {
      _handleResolution(value,
          SystemBarcode(barcode: picked.systemBarcode ?? value, itemId: picked.id));
    }
  }

  void _addLog(IconData icon, String message,
      {bool isError = false, String? itemId}) {
    setState(() {
      _log.insert(0, _LogEntry(
        icon: icon,
        message: message,
        isError: isError,
        time: DateTime.now(),
        itemId: itemId,
      ));
      if (_log.length > 50) _log.removeLast();
    });
  }

  Future<void> _refreshSession() async {
    try {
      final s = await widget.api.getSession(_session.id);
      if (mounted) setState(() => _session = s);
    } catch (_) {}
  }

  Future<void> _endSession() async {
    final confirm = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('End Session?'),
        content: Text(
            'Session has ${_session.totalItems} items. '
            'This will end the session and revoke any camera links.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('End Session'),
          ),
        ],
      ),
    );
    if (confirm != true || !mounted) return;

    try {
      await widget.api.endSession(_session.id);
      if (mounted) Navigator.pop(context);
    } on ApiError catch (e) {
      if (mounted) _snack('Failed to end session: ${e.message}');
    }
  }

  void _snack(String msg) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }

  Future<void> _takePhotoForItem(String itemId) async {
    _stopCamera();
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
    }
    if (mounted && _mode == _InputMode.camera) _startCamera();

    if (photo == null || !mounted) return;
    _snack('Uploading photo…');
    try {
      await widget.api.uploadImage(itemId, File(photo.path));
      if (mounted) _snack('Photo uploaded');
    } on ApiError catch (e) {
      if (mounted) _snack('Upload failed: ${e.message}');
    }
  }

  // ── Build ─────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(
        title: Text(_session.displayName),
        actions: [
          IconButton(
            icon: const Icon(Icons.stop_circle_outlined),
            tooltip: 'End session',
            onPressed: _endSession,
          ),
        ],
      ),
      body: Column(
        children: [
          // Active container bar
          _buildContextBar(theme),
          // Stats bar
          _buildStatsBar(theme),
          const Divider(height: 1),
          // Scanner toggle
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
            child: SegmentedButton<_InputMode>(
              segments: const [
                ButtonSegment(
                  value: _InputMode.camera,
                  icon: Icon(Icons.camera_alt, size: 18),
                  label: Text('Camera'),
                ),
                ButtonSegment(
                  value: _InputMode.bluetooth,
                  icon: Icon(Icons.bluetooth, size: 18),
                  label: Text('Scanner'),
                ),
              ],
              selected: {_mode},
              onSelectionChanged: (s) => _setMode(s.first),
            ),
          ),
          // Scanner input
          if (_mode == _InputMode.camera)
            _buildCameraPreview(theme)
          else
            _buildScannerCard(theme),
          // Processing indicator
          if (_processing)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 8),
              child: LinearProgressIndicator(),
            ),
          // Scan log
          Expanded(
            child: _log.isEmpty
                ? Center(
                    child: Text(
                      _activeContainerId == null
                          ? 'Pick a container to start stocking'
                          : 'Scan items to place them',
                      style: theme.textTheme.bodyMedium?.copyWith(
                        color: theme.colorScheme.onSurface.withValues(alpha: 0.4),
                      ),
                    ),
                  )
                : ListView.builder(
                    itemCount: _log.length,
                    padding: const EdgeInsets.symmetric(vertical: 4),
                    itemBuilder: (_, i) => _buildLogEntry(_log[i], theme),
                  ),
          ),
        ],
      ),
    );
  }

  Widget _buildContextBar(ThemeData theme) {
    final hasContext = _activeContainerId != null;
    return InkWell(
      onTap: _showContainerPicker,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        color: hasContext
            ? theme.colorScheme.primaryContainer
            : theme.colorScheme.errorContainer,
        child: Row(
          children: [
            Icon(
              hasContext ? Icons.folder_open : Icons.folder_off_outlined,
              size: 20,
              color: hasContext
                  ? theme.colorScheme.onPrimaryContainer
                  : theme.colorScheme.onErrorContainer,
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                hasContext
                    ? _activeContainerName ?? 'Container'
                    : 'No container selected — tap to pick one',
                style: TextStyle(
                  fontWeight: FontWeight.w600,
                  color: hasContext
                      ? theme.colorScheme.onPrimaryContainer
                      : theme.colorScheme.onErrorContainer,
                  fontSize: 14,
                ),
              ),
            ),
            Icon(
              Icons.swap_horiz,
              size: 18,
              color: hasContext
                  ? theme.colorScheme.onPrimaryContainer
                  : theme.colorScheme.onErrorContainer,
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildStatsBar(ThemeData theme) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
      child: Row(
        children: [
          _statChip(theme, Icons.add_circle_outline,
              '${_session.itemsCreated}', 'created'),
          const SizedBox(width: 16),
          _statChip(theme, Icons.move_to_inbox,
              '${_session.itemsMoved}', 'moved'),
          if (_session.itemsErrored > 0) ...[
            const SizedBox(width: 16),
            _statChip(theme, Icons.error_outline,
                '${_session.itemsErrored}', 'errors'),
          ],
        ],
      ),
    );
  }

  Widget _statChip(
      ThemeData theme, IconData icon, String count, String label) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 14,
            color: theme.colorScheme.onSurface.withValues(alpha: 0.5)),
        const SizedBox(width: 4),
        Text('$count $label',
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurface.withValues(alpha: 0.6),
            )),
      ],
    );
  }

  Widget _buildCameraPreview(ThemeData theme) {
    return Padding(
      padding: const EdgeInsets.all(12),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(12),
        child: SizedBox(
          height: 180,
          child: _cameraController == null
              ? const Center(child: CircularProgressIndicator())
              : Stack(
                  children: [
                    MobileScanner(
                      controller: _cameraController!,
                      onDetect: _onCameraDetect,
                      errorBuilder: (context, error, child) => Center(
                        child: Text(
                          'Camera unavailable: ${error.errorCode.name}',
                          textAlign: TextAlign.center,
                          style: TextStyle(color: theme.colorScheme.onSurface),
                        ),
                      ),
                    ),
                    Positioned(
                      right: 8,
                      bottom: 8,
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          IconButton.filledTonal(
                            onPressed: () =>
                                _cameraController?.toggleTorch(),
                            icon: const Icon(Icons.flash_on, size: 18),
                            style: IconButton.styleFrom(
                              backgroundColor:
                                  Colors.black.withValues(alpha: 0.5),
                              foregroundColor: Colors.white,
                            ),
                          ),
                          const SizedBox(width: 4),
                          IconButton.filledTonal(
                            onPressed: () =>
                                _cameraController?.switchCamera(),
                            icon: const Icon(Icons.cameraswitch, size: 18),
                            style: IconButton.styleFrom(
                              backgroundColor:
                                  Colors.black.withValues(alpha: 0.5),
                              foregroundColor: Colors.white,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
        ),
      ),
    );
  }

  Widget _buildScannerCard(ThemeData theme) {
    final connected = _scannerState == BtScannerState.connected;
    final connecting = _scannerState == BtScannerState.connecting;

    final IconData icon;
    final String title;
    final String subtitle;

    if (connected) {
      icon = Icons.bluetooth_connected;
      title = _scanner.device?.name ?? 'Scanner';
      subtitle = 'Ready — pull the trigger';
    } else if (connecting) {
      icon = Icons.bluetooth_searching;
      title = 'Connecting…';
      subtitle = _scanner.device?.name ?? '';
    } else {
      icon = Icons.bluetooth_disabled;
      title = _savedBtName ?? 'Bluetooth scanner';
      subtitle = _savedBtAddress != null
          ? 'Tap Connect to reconnect'
          : 'Not connected';
    }

    return Padding(
      padding: const EdgeInsets.all(12),
      child: Card(
        color: theme.colorScheme.surfaceContainerHighest,
        elevation: 0,
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Row(
            children: [
              Icon(icon, size: 22),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(title,
                        style: const TextStyle(
                            fontWeight: FontWeight.bold, fontSize: 13)),
                    Text(subtitle, style: const TextStyle(fontSize: 11)),
                  ],
                ),
              ),
              if (connected)
                TextButton(
                  onPressed: () => _scanner.disconnect(),
                  child: const Text('Disconnect'),
                )
              else if (!connecting) ...[
                TextButton(
                  onPressed: _changeScanner,
                  child: Text(_savedBtAddress != null ? 'Change' : 'Pick'),
                ),
                if (_savedBtAddress != null)
                  TextButton(
                    onPressed: _connectScanner,
                    child: const Text('Connect'),
                  ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildLogEntry(_LogEntry entry, ThemeData theme) {
    final timeStr =
        '${entry.time.hour.toString().padLeft(2, '0')}:'
        '${entry.time.minute.toString().padLeft(2, '0')}:'
        '${entry.time.second.toString().padLeft(2, '0')}';
    final tappable = entry.itemId != null;
    final row = Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(
            entry.icon,
            size: 16,
            color: entry.isError
                ? theme.colorScheme.error
                : theme.colorScheme.onSurface.withValues(alpha: 0.5),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              entry.message,
              style: TextStyle(
                fontSize: 12,
                color: entry.isError
                    ? theme.colorScheme.error
                    : theme.colorScheme.onSurface.withValues(alpha: 0.8),
                decoration:
                    tappable ? TextDecoration.underline : TextDecoration.none,
              ),
            ),
          ),
          if (tappable)
            GestureDetector(
              onTap: () => _takePhotoForItem(entry.itemId!),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 4),
                child: Icon(Icons.camera_alt, size: 14,
                    color: theme.colorScheme.primary.withValues(alpha: 0.7)),
              ),
            ),
          if (tappable)
            Icon(Icons.open_in_new, size: 12,
                color: theme.colorScheme.onSurface.withValues(alpha: 0.3)),
          const SizedBox(width: 4),
          Text(timeStr,
              style: TextStyle(
                fontSize: 10,
                color: theme.colorScheme.onSurface.withValues(alpha: 0.3),
              )),
        ],
      ),
    );
    if (!tappable) return row;
    return GestureDetector(
      onTap: () => Navigator.of(context).push(
        MaterialPageRoute(
          builder: (_) => ItemDetailScreen(
            api: widget.api,
            itemId: entry.itemId!,
          ),
        ),
      ),
      child: row,
    );
  }
}

// ── Container picker bottom sheet ─────────────────────────────────────

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
    // Load recent containers on open
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
        // Handle bar
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
        if (_searching)
          const LinearProgressIndicator(),
        Expanded(
          child: _results == null || _results!.isEmpty
              ? Center(
                  child: Text(
                    _results == null ? '' : 'No containers found',
                    style: theme.textTheme.bodyMedium?.copyWith(
                      color: theme.colorScheme.onSurface.withValues(alpha: 0.4),
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

class _LogEntry {
  final IconData icon;
  final String message;
  final bool isError;
  final DateTime time;
  final String? itemId;

  const _LogEntry({
    required this.icon,
    required this.message,
    this.isError = false,
    required this.time,
    this.itemId,
  });
}
