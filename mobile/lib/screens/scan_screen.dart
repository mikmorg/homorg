import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_bluetooth_serial/flutter_bluetooth_serial.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../models/item.dart';
import '../services/homorg_api.dart';
import '../services/bluetooth_scanner_service.dart';
import 'item_detail_screen.dart';

enum _ScanMode { camera, bluetooth }

const _btAddressPrefKey = 'last_bt_address';
const _btNamePrefKey = 'last_bt_name';

class ScanScreen extends StatefulWidget {
  final HomorgApi api;
  final bool isActive;

  const ScanScreen({super.key, required this.api, this.isActive = true});

  @override
  State<ScanScreen> createState() => _ScanScreenState();
}

class _ScanScreenState extends State<ScanScreen> {
  final _scanner = BluetoothScannerService();
  StreamSubscription<String>? _scanSub;
  StreamSubscription<BtScannerState>? _scanStateSub;
  BtScannerState _scannerState = BtScannerState.disconnected;

  String? _savedBtAddress;
  String? _savedBtName;

  // Scan mode
  _ScanMode _mode = _ScanMode.camera;
  MobileScannerController? _cameraController;

  // Resolution state
  bool _resolving = false;
  String? _lastBarcode;
  BarcodeResolution? _lastResolution;
  String? _resolveError;

  // Search state
  final _searchController = TextEditingController();
  final _searchFocus = FocusNode();
  Timer? _searchDebounce;
  List<ItemSummary>? _searchResults;
  bool _searching = false;
  bool _searchActive = false;

  // Recent scans
  final List<_ScanEntry> _recentScans = [];

  @override
  void initState() {
    super.initState();
    _scanSub = _scanner.scans.listen(_onBarcode);
    _scanStateSub = _scanner.stateStream.listen((s) {
      if (!mounted) return;
      setState(() => _scannerState = s);
    });
    _loadSavedBtDevice();
    if (widget.isActive && _mode == _ScanMode.camera) {
      _startCamera();
    }
  }

  @override
  void didUpdateWidget(covariant ScanScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.isActive != oldWidget.isActive) {
      if (widget.isActive && _mode == _ScanMode.camera) {
        _startCamera();
      } else if (!widget.isActive) {
        _stopCamera();
      }
    }
  }

  @override
  void dispose() {
    _scanSub?.cancel();
    _scanStateSub?.cancel();
    _scanner.dispose();
    _cameraController?.dispose();
    _searchController.dispose();
    _searchFocus.dispose();
    _searchDebounce?.cancel();
    super.dispose();
  }

  void _startCamera() {
    _cameraController = MobileScannerController(
      detectionSpeed: DetectionSpeed.noDuplicates,
    );
  }

  void _stopCamera() {
    _cameraController?.dispose();
    _cameraController = null;
  }

  void _setMode(_ScanMode mode) {
    if (mode == _mode) return;
    setState(() => _mode = mode);
    if (mode == _ScanMode.camera) {
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

  Future<void> _loadSavedBtDevice() async {
    final prefs = await SharedPreferences.getInstance();
    if (!mounted) return;
    final addr = prefs.getString(_btAddressPrefKey);
    final name = prefs.getString(_btNamePrefKey);
    setState(() {
      _savedBtAddress = addr;
      _savedBtName = name;
    });
    // Default to BT mode if a scanner was previously paired
    if (addr != null) {
      _setMode(_ScanMode.bluetooth);
    }
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

  Future<void> _onBarcode(String barcode) async {
    if (_resolving) return;

    setState(() {
      _resolving = true;
      _lastBarcode = barcode;
      _lastResolution = null;
      _resolveError = null;
    });

    try {
      final resolution = await widget.api.resolveBarcode(barcode);
      if (!mounted) return;

      setState(() {
        _resolving = false;
        _lastResolution = resolution;
        _recentScans.insert(0, _ScanEntry(barcode: barcode, resolution: resolution));
        if (_recentScans.length > 20) _recentScans.removeLast();
      });

      // Auto-navigate for system barcodes
      if (resolution is SystemBarcode) {
        _navigateToItem(resolution.itemId);
      }
    } on ApiError catch (e) {
      if (!mounted) return;
      setState(() {
        _resolving = false;
        _resolveError = e.message;
        _recentScans.insert(0, _ScanEntry(barcode: barcode, error: e.message));
        if (_recentScans.length > 20) _recentScans.removeLast();
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _resolving = false;
        _resolveError = 'Resolve failed';
      });
    }
  }

  Future<void> _navigateToItem(String itemId) async {
    _stopCamera();
    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => ItemDetailScreen(api: widget.api, itemId: itemId),
      ),
    );
    if (mounted && widget.isActive && _mode == _ScanMode.camera) {
      _startCamera();
    }
  }

  Future<void> _showItemPicker(List<String> itemIds) async {
    List<Item> items;
    try {
      items = await Future.wait(itemIds.map(widget.api.getItem));
    } catch (_) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Failed to load items')),
        );
      }
      return;
    }
    if (!mounted) return;

    final picked = await showModalBottomSheet<String>(
      context: context,
      isScrollControlled: true,
      builder: (ctx) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const ListTile(
              title: Text('Multiple items match',
                  style: TextStyle(fontWeight: FontWeight.bold)),
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
                  trailing: const Icon(Icons.chevron_right, size: 16),
                  onTap: () => Navigator.pop(ctx, item.id),
                )),
          ],
        ),
      ),
    );
    if (picked != null && mounted) _navigateToItem(picked);
  }

  // ── Search ─────────────────────────────────────────────────────────

  void _onSearchChanged(String query) {
    _searchDebounce?.cancel();
    if (query.trim().isEmpty) {
      setState(() {
        _searchResults = null;
        _searching = false;
      });
      return;
    }
    _searchDebounce = Timer(const Duration(milliseconds: 300), () {
      _doSearch(query.trim());
    });
  }

  Future<void> _doSearch(String query) async {
    setState(() => _searching = true);
    try {
      final results = await widget.api.search(query);
      if (!mounted) return;
      setState(() {
        _searchResults = results;
        _searching = false;
      });
    } on ApiError {
      if (mounted) setState(() => _searching = false);
    }
  }

  void _openSearch() {
    setState(() => _searchActive = true);
    _searchFocus.requestFocus();
  }

  void _closeSearch() {
    setState(() {
      _searchActive = false;
      _searchResults = null;
      _searchController.clear();
    });
    _searchFocus.unfocus();
  }

  // ── BT scanner connection ──────────────────────────────────────────

  Future<void> _connectScanner() async {
    final granted = await BluetoothScannerService.ensurePermissions();
    if (!mounted) return;
    if (!granted) {
      _showSnack('Bluetooth permissions denied');
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
        if (mounted) _showSnack('Connect failed: $e');
      }
      return;
    }

    await _showDevicePicker();
  }

  Future<void> _changeScanner() async {
    final granted = await BluetoothScannerService.ensurePermissions();
    if (!mounted) return;
    if (!granted) {
      _showSnack('Bluetooth permissions denied');
      return;
    }

    if (_scanner.isConnected) {
      await _scanner.disconnect();
    }

    await _showDevicePicker();
  }

  Future<void> _showDevicePicker() async {
    List<BluetoothDevice> devices;
    try {
      devices = await _scanner.bondedDevices();
    } catch (e) {
      if (mounted) _showSnack('Bluetooth unavailable: $e');
      return;
    }
    if (!mounted) return;
    if (devices.isEmpty) {
      _showSnack('No paired devices — pair your scanner in Android settings first');
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
      if (mounted) _showSnack('Connect failed: $e');
    }
  }

  Future<void> _disconnectScanner() async {
    await _scanner.disconnect();
  }

  void _showSnack(String msg) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }

  // ── Build ─────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Column(
      children: [
        // Search bar
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
          child: TextField(
            controller: _searchController,
            focusNode: _searchFocus,
            decoration: InputDecoration(
              hintText: 'Search items…',
              prefixIcon: const Icon(Icons.search, size: 20),
              suffixIcon: _searchActive
                  ? IconButton(
                      icon: const Icon(Icons.close, size: 20),
                      onPressed: _closeSearch,
                    )
                  : null,
              border: const OutlineInputBorder(),
              isDense: true,
              contentPadding:
                  const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            ),
            onChanged: _onSearchChanged,
            onTap: _openSearch,
          ),
        ),
        // Search results overlay (replaces scanner when active)
        if (_searchActive) ...[
          if (_searching)
            const Padding(
              padding: EdgeInsets.only(top: 4),
              child: LinearProgressIndicator(),
            ),
          if (_searchResults != null)
            Expanded(
              child: _searchResults!.isEmpty
                  ? Center(
                      child: Text('No results',
                          style: theme.textTheme.bodyMedium?.copyWith(
                            color: theme.colorScheme.onSurface
                                .withValues(alpha: 0.4),
                          )),
                    )
                  : ListView.builder(
                      itemCount: _searchResults!.length,
                      itemBuilder: (_, i) =>
                          _buildSearchResult(_searchResults![i], theme),
                    ),
            ),
          if (_searchResults == null && !_searching)
            const Expanded(child: SizedBox()),
        ] else ...[
        // Mode toggle
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
          child: SegmentedButton<_ScanMode>(
            segments: const [
              ButtonSegment(
                value: _ScanMode.camera,
                icon: Icon(Icons.camera_alt, size: 18),
                label: Text('Camera'),
              ),
              ButtonSegment(
                value: _ScanMode.bluetooth,
                icon: Icon(Icons.bluetooth, size: 18),
                label: Text('Scanner'),
              ),
            ],
            selected: {_mode},
            onSelectionChanged: (s) => _setMode(s.first),
          ),
        ),
        if (_mode == _ScanMode.camera)
          _buildCameraPreview(theme)
        else
          _buildScannerCard(theme),
        if (_resolving)
          const Padding(
            padding: EdgeInsets.all(24),
            child: CircularProgressIndicator(),
          ),
        if (_lastResolution != null && !_resolving)
          _buildResolutionCard(theme),
        if (_resolveError != null && !_resolving)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
            child: Container(
              padding:
                  const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
              decoration: BoxDecoration(
                color: theme.colorScheme.errorContainer,
                borderRadius: BorderRadius.circular(8),
              ),
              child: Row(
                children: [
                  Icon(Icons.error_outline,
                      color: theme.colorScheme.onErrorContainer, size: 18),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      '$_resolveError ($_lastBarcode)',
                      style: TextStyle(
                        color: theme.colorScheme.onErrorContainer,
                        fontSize: 13,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        const SizedBox(height: 8),
        if (_recentScans.isNotEmpty) ...[
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Row(
              children: [
                Text('Recent scans', style: theme.textTheme.labelLarge),
                const Spacer(),
                TextButton(
                  onPressed: () => setState(() => _recentScans.clear()),
                  child: const Text('Clear'),
                ),
              ],
            ),
          ),
          Expanded(
            child: ListView.builder(
              itemCount: _recentScans.length,
              itemBuilder: (_, i) => _buildScanEntry(_recentScans[i], theme),
            ),
          ),
        ],
        if (_recentScans.isEmpty && !_resolving)
          Expanded(
            child: Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(Icons.qr_code_scanner_rounded,
                      size: 64,
                      color: theme.colorScheme.onSurface.withValues(alpha: 0.2)),
                  const SizedBox(height: 12),
                  Text(
                    'Scan a barcode to get started',
                    style: theme.textTheme.bodyMedium?.copyWith(
                      color: theme.colorScheme.onSurface.withValues(alpha: 0.4),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ], // end else (scanner mode)
      ],
    );
  }

  Widget _buildSearchResult(ItemSummary item, ThemeData theme) {
    return ListTile(
      leading: Icon(
        item.isContainer ? Icons.folder : Icons.inventory_2,
        size: 20,
        color: theme.colorScheme.primary,
      ),
      title: Text(item.displayName, style: const TextStyle(fontSize: 14)),
      subtitle: item.parentName != null
          ? Text('in ${item.parentName}', style: theme.textTheme.bodySmall)
          : null,
      trailing: const Icon(Icons.arrow_forward_ios, size: 12),
      dense: true,
      onTap: () {
        _closeSearch();
        _navigateToItem(item.id);
      },
    );
  }

  Widget _buildCameraPreview(ThemeData theme) {
    return Padding(
      padding: const EdgeInsets.all(16),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(12),
        child: SizedBox(
          height: 240,
          child: _cameraController == null
              ? const Center(child: CircularProgressIndicator())
              : Stack(
                  children: [
                    MobileScanner(
                      controller: _cameraController!,
                      onDetect: _onCameraDetect,
                      errorBuilder: (context, error, child) => Center(
                        child: Padding(
                          padding: const EdgeInsets.all(24),
                          child: Text(
                            'Camera unavailable: ${error.errorCode.name}',
                            textAlign: TextAlign.center,
                            style: TextStyle(
                              color: theme.colorScheme.onSurface,
                            ),
                          ),
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
                            icon: const Icon(Icons.flash_on, size: 20),
                            tooltip: 'Toggle torch',
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
                            icon:
                                const Icon(Icons.cameraswitch, size: 20),
                            tooltip: 'Switch camera',
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
    final Color bg;
    final Color fg;

    if (connected) {
      icon = Icons.bluetooth_connected;
      title = _scanner.device?.name ?? 'Scanner';
      subtitle = 'Ready — pull the trigger';
      bg = theme.colorScheme.secondaryContainer;
      fg = theme.colorScheme.onSecondaryContainer;
    } else if (connecting) {
      icon = Icons.bluetooth_searching;
      title = 'Connecting…';
      subtitle = _scanner.device?.name ?? '';
      bg = theme.colorScheme.surfaceContainerHighest;
      fg = theme.colorScheme.onSurface;
    } else {
      icon = Icons.bluetooth_disabled;
      title = _savedBtName ?? 'Bluetooth scanner';
      subtitle = _scanner.lastError != null
          ? 'Error: ${_scanner.lastError}'
          : (_savedBtAddress != null
              ? 'Tap Connect to reconnect'
              : 'Not connected');
      bg = theme.colorScheme.surfaceContainerHighest;
      fg = theme.colorScheme.onSurface.withValues(alpha: 0.7);
    }

    return Padding(
      padding: const EdgeInsets.all(16),
      child: Card(
        color: bg,
        elevation: 0,
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Row(
            children: [
              Icon(icon, color: fg, size: 26),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(title,
                        style: TextStyle(
                            color: fg,
                            fontWeight: FontWeight.bold,
                            fontSize: 14)),
                    const SizedBox(height: 2),
                    Text(subtitle,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(color: fg, fontSize: 12)),
                  ],
                ),
              ),
              const SizedBox(width: 8),
              if (connected)
                TextButton(
                  onPressed: _disconnectScanner,
                  child: const Text('Disconnect'),
                )
              else ...[
                if (!connecting)
                  TextButton(
                    onPressed: _changeScanner,
                    child:
                        Text(_savedBtAddress != null ? 'Change' : 'Connect'),
                  ),
                if (_savedBtAddress != null)
                  TextButton(
                    onPressed: connecting ? null : _connectScanner,
                    child: const Text('Connect'),
                  ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildResolutionCard(ThemeData theme) {
    final r = _lastResolution!;
    String title;
    String subtitle;
    IconData icon;
    VoidCallback? onTap;

    switch (r) {
      case SystemBarcode(:final barcode, :final itemId):
        title = barcode;
        subtitle = 'System barcode — tap to view';
        icon = Icons.inventory_2;
        onTap = () => _navigateToItem(itemId);
      case ExternalCode(:final codeType, :final value, :final itemIds):
        title = '$codeType: $value';
        subtitle = itemIds.isEmpty
            ? 'No items linked'
            : '${itemIds.length} item(s) linked';
        icon = Icons.qr_code;
        if (itemIds.length == 1) {
          onTap = () => _navigateToItem(itemIds[0]);
        } else if (itemIds.length > 1) {
          onTap = () => _showItemPicker(itemIds);
        }
      case Preset(:final barcode, :final isContainer):
        title = barcode;
        subtitle = isContainer ? 'Container preset' : 'Item preset';
        icon = Icons.label_outline;
      case UnknownSystem(:final barcode):
        title = barcode;
        subtitle = 'Unassigned system barcode';
        icon = Icons.help_outline;
      case Unknown(:final value):
        title = value;
        subtitle = 'Unknown code';
        icon = Icons.help_outline;
    }

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: Card(
        child: ListTile(
          leading: Icon(icon, color: theme.colorScheme.primary),
          title: Text(title, style: const TextStyle(fontSize: 14)),
          subtitle: Text(subtitle),
          trailing: onTap != null
              ? const Icon(Icons.arrow_forward_ios, size: 14)
              : null,
          onTap: onTap,
        ),
      ),
    );
  }

  Widget _buildScanEntry(_ScanEntry entry, ThemeData theme) {
    if (entry.error != null) {
      return ListTile(
        dense: true,
        leading:
            Icon(Icons.error_outline, size: 18, color: theme.colorScheme.error),
        title: Text(entry.barcode,
            style: const TextStyle(fontSize: 13, fontFamily: 'monospace')),
        subtitle: Text(entry.error!,
            style: TextStyle(fontSize: 11, color: theme.colorScheme.error)),
      );
    }

    final r = entry.resolution;
    IconData icon = Icons.qr_code;
    String? itemId;

    if (r is SystemBarcode) {
      icon = Icons.inventory_2;
      itemId = r.itemId;
    } else if (r is ExternalCode && r.itemIds.length == 1) {
      icon = Icons.qr_code;
      itemId = r.itemIds[0];
    }

    return ListTile(
      dense: true,
      leading: Icon(icon, size: 18),
      title: Text(entry.barcode,
          style: const TextStyle(fontSize: 13, fontFamily: 'monospace')),
      trailing: itemId != null
          ? const Icon(Icons.arrow_forward_ios, size: 12)
          : null,
      onTap: itemId != null ? () => _navigateToItem(itemId!) : null,
    );
  }
}

class _ScanEntry {
  final String barcode;
  final BarcodeResolution? resolution;
  final String? error;

  const _ScanEntry({required this.barcode, this.resolution, this.error});
}
