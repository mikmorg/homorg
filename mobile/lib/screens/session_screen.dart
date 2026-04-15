import 'dart:async';
import 'dart:io';

import 'package:camera/camera.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bluetooth_serial/flutter_bluetooth_serial.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../models/camera_models.dart';
import '../services/api_service.dart';
import '../services/bluetooth_scanner_service.dart';
import 'camera_capture_screen.dart';

const _autoOpenCameraPrefKey = 'auto_open_camera';

typedef ApiServiceFactory = ApiService Function(CameraConnection);

class SessionScreen extends StatefulWidget {
  final CameraConnection connection;
  final ApiServiceFactory? apiServiceFactory;

  const SessionScreen({super.key, required this.connection, this.apiServiceFactory});

  @override
  State<SessionScreen> createState() => _SessionScreenState();
}

class _SessionScreenState extends State<SessionScreen> {
  late final ApiService _api;
  SessionStatus? _status;
  bool _loadingStatus = true;
  String? _statusError;
  bool _tokenExpired = false;

  bool _uploading = false;
  String? _uploadMessage;
  bool _uploadSuccess = false;

  bool _autoOpenCamera = false;
  bool _cameraOpen = false;
  String? _lastSeenItemId;
  Timer? _pollTimer;

  final _scanner = BluetoothScannerService();
  StreamSubscription<String>? _scanSub;
  StreamSubscription<BtScannerState>? _scanStateSub;
  BtScannerState _scannerState = BtScannerState.disconnected;
  String? _lastScan;
  String? _lastScanError;

  @override
  void initState() {
    super.initState();
    _api = widget.apiServiceFactory != null
        ? widget.apiServiceFactory!(widget.connection)
        : ApiService(widget.connection);
    _scanSub = _scanner.scans.listen(_onScannerBarcode);
    _scanStateSub = _scanner.stateStream.listen((s) {
      if (!mounted) return;
      setState(() => _scannerState = s);
    });
    _loadAutoOpenPref();
    _fetchStatus();
    _pollTimer = Timer.periodic(
      const Duration(seconds: 2),
      (_) => _fetchStatus(),
    );
  }

  Future<void> _loadAutoOpenPref() async {
    final prefs = await SharedPreferences.getInstance();
    if (!mounted) return;
    setState(() {
      _autoOpenCamera = prefs.getBool(_autoOpenCameraPrefKey) ?? false;
    });
  }

  Future<void> _setAutoOpenPref(bool v) async {
    setState(() => _autoOpenCamera = v);
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool(_autoOpenCameraPrefKey, v);
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    _scanSub?.cancel();
    _scanStateSub?.cancel();
    _scanner.dispose();
    super.dispose();
  }

  Future<void> _onScannerBarcode(String barcode) async {
    try {
      await _api.sendBarcode(barcode);
      if (!mounted) return;
      setState(() {
        _lastScan = barcode;
        _lastScanError = null;
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _lastScan = barcode;
        _lastScanError = e.message;
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _lastScan = barcode;
        _lastScanError = 'Failed to send';
      });
    }
  }

  Future<void> _pickScanner() async {
    final granted = await BluetoothScannerService.ensurePermissions();
    if (!mounted) return;
    if (!granted) {
      _showSnack('Bluetooth permissions denied');
      return;
    }

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
              title: Text('Paired devices', style: TextStyle(fontWeight: FontWeight.bold)),
              dense: true,
            ),
            for (final d in devices)
              ListTile(
                leading: const Icon(Icons.bluetooth),
                title: Text(d.name ?? '(unnamed)'),
                subtitle: Text(d.address),
                onTap: () => Navigator.of(ctx).pop(d),
              ),
          ],
        ),
      ),
    );
    if (selected == null || !mounted) return;

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

  Future<void> _fetchStatus() async {
    if (_tokenExpired) return;
    try {
      final status = await _api.getStatus();
      if (!mounted) return;
      final newItemId = status.activeItemId;
      final changed = newItemId != null && newItemId != _lastSeenItemId;
      setState(() {
        _status = status;
        _loadingStatus = false;
        _statusError = null;
        _lastSeenItemId = newItemId;
      });
      if (changed &&
          _autoOpenCamera &&
          !_cameraOpen &&
          !_uploading &&
          !status.sessionEnded) {
        _takePhoto();
      }
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _statusError = e.message;
        _loadingStatus = false;
        if (e.statusCode == 401) _tokenExpired = true;
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _statusError ??= 'Trying to reconnect…';
        _loadingStatus = false;
      });
    }
  }

  Future<void> _takePhoto() async {
    if (_cameraOpen) return;
    _cameraOpen = true;
    XFile? photo;
    try {
      photo = await Navigator.of(context).push<XFile>(
        MaterialPageRoute(
          builder: (_) => const CameraCaptureScreen(),
          fullscreenDialog: true,
        ),
      );
    } catch (_) {
      _cameraOpen = false;
      if (mounted) {
        setState(() {
          _uploadSuccess = false;
          _uploadMessage = 'Camera access denied';
        });
      }
      return;
    }
    _cameraOpen = false;

    if (photo == null || !mounted) return;

    setState(() {
      _uploading = true;
      _uploadMessage = null;
    });

    try {
      final result = await _api.uploadImage(File(photo.path));
      if (!mounted) return;
      setState(() {
        _uploading = false;
        _uploadSuccess = true;
        _uploadMessage =
            'Photo attached (${result.imageCount} total on this item)';
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _uploading = false;
        _uploadSuccess = false;
        _uploadMessage = e.message;
        if (e.statusCode == 401) {
          _tokenExpired = true;
        }
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _uploading = false;
        _uploadSuccess = false;
        _uploadMessage = 'Upload failed — check connection';
      });
    }
  }

  // ── Build ──────────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Stocker Session'),
        actions: [
          Row(
            children: [
              const Tooltip(
                message: 'Auto-open camera on new item',
                child: Icon(Icons.bolt, size: 20),
              ),
              Switch(
                value: _autoOpenCamera,
                onChanged: _setAutoOpenPref,
              ),
            ],
          ),
          if (!_tokenExpired)
            IconButton(
              icon: const Icon(Icons.refresh),
              onPressed: _fetchStatus,
              tooltip: 'Refresh',
            ),
        ],
      ),
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _buildStatusCard(),
              const SizedBox(height: 28),
              _buildPhotoButton(),
              if (_uploadMessage != null) ...[
                const SizedBox(height: 16),
                _buildUploadBanner(),
              ],
              const SizedBox(height: 28),
              _buildScannerCard(),
              const Spacer(),
              _buildConnectionInfo(),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildStatusCard() {
    final theme = Theme.of(context);

    if (_loadingStatus) {
      return const Card(
        child: Padding(
          padding: EdgeInsets.all(24),
          child: Center(child: CircularProgressIndicator()),
        ),
      );
    }

    if (_tokenExpired) {
      return _StatusCard(
        color: theme.colorScheme.errorContainer,
        onColor: theme.colorScheme.onErrorContainer,
        icon: Icons.key_off_outlined,
        title: 'Token expired',
        subtitle: 'Go back and generate a new camera link',
      );
    }

    if (_status == null || _statusError != null) {
      return _StatusCard(
        color: theme.colorScheme.errorContainer,
        onColor: theme.colorScheme.onErrorContainer,
        icon: Icons.wifi_off,
        title: 'Cannot reach server',
        subtitle: _statusError ?? 'Check that the backend is running',
      );
    }

    if (_status!.sessionEnded) {
      return _StatusCard(
        color: theme.colorScheme.errorContainer,
        onColor: theme.colorScheme.onErrorContainer,
        icon: Icons.stop_circle_outlined,
        title: 'Session ended',
        subtitle: 'The stocker session has been closed',
      );
    }

    return _StatusCard(
      color: theme.colorScheme.primaryContainer,
      onColor: theme.colorScheme.onPrimaryContainer,
      icon: Icons.check_circle_outline_rounded,
      title: 'Connected',
      subtitle: 'Ready to take photos',
    );
  }

  Widget _buildPhotoButton() {
    final theme = Theme.of(context);
    final canPhoto = !_uploading &&
        !_tokenExpired &&
        _status != null &&
        !_status!.sessionEnded;

    if (_uploading) {
      return Column(
        children: [
          const CircularProgressIndicator(),
          const SizedBox(height: 12),
          Text('Uploading…', style: theme.textTheme.bodyMedium),
        ],
      );
    }

    return SizedBox(
      height: 100,
      child: FilledButton.icon(
        onPressed: canPhoto ? _takePhoto : null,
        icon: const Icon(Icons.camera_alt_rounded, size: 28),
        label: const Text('Take Photo', style: TextStyle(fontSize: 18)),
        style: FilledButton.styleFrom(
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(16),
          ),
        ),
      ),
    );
  }

  Widget _buildUploadBanner() {
    final theme = Theme.of(context);
    final color = _uploadSuccess
        ? theme.colorScheme.secondaryContainer
        : theme.colorScheme.errorContainer;
    final onColor = _uploadSuccess
        ? theme.colorScheme.onSecondaryContainer
        : theme.colorScheme.onErrorContainer;
    final icon = _uploadSuccess ? Icons.check_circle : Icons.error_outline;

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Row(
        children: [
          Icon(icon, color: onColor, size: 20),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              _uploadMessage!,
              style: TextStyle(color: onColor, fontSize: 13),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildScannerCard() {
    final theme = Theme.of(context);
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
      subtitle = _lastScan == null
          ? 'Ready — pull the trigger'
          : (_lastScanError != null
              ? 'Send failed: $_lastScanError'
              : 'Sent: $_lastScan');
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
      title = 'Bluetooth scanner';
      subtitle = _scanner.lastError != null
          ? 'Error: ${_scanner.lastError}'
          : 'Not connected';
      bg = theme.colorScheme.surfaceContainerHighest;
      fg = theme.colorScheme.onSurface.withValues(alpha: 0.7);
    }

    return Card(
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
                          color: fg, fontWeight: FontWeight.bold, fontSize: 14)),
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
            else
              TextButton(
                onPressed: connecting ? null : _pickScanner,
                child: const Text('Connect'),
              ),
          ],
        ),
      ),
    );
  }

  Widget _buildConnectionInfo() {
    final theme = Theme.of(context);
    return Text(
      widget.connection.baseUrl,
      textAlign: TextAlign.center,
      style: theme.textTheme.bodySmall?.copyWith(
        color: theme.colorScheme.onSurface.withValues(alpha: 0.35),
      ),
    );
  }
}

// ── Helper widget ─────────────────────────────────────────────────────────

class _StatusCard extends StatelessWidget {
  final Color color;
  final Color onColor;
  final IconData icon;
  final String title;
  final String subtitle;

  const _StatusCard({
    required this.color,
    required this.onColor,
    required this.icon,
    required this.title,
    required this.subtitle,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      color: color,
      elevation: 0,
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Row(
          children: [
            Icon(icon, color: onColor, size: 28),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    title,
                    style: TextStyle(
                      color: onColor,
                      fontWeight: FontWeight.bold,
                      fontSize: 15,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    subtitle,
                    style: TextStyle(color: onColor, fontSize: 13),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
