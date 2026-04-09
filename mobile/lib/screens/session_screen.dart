import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';

import '../models/camera_models.dart';
import '../services/api_service.dart';

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

  Timer? _pollTimer;
  final _picker = ImagePicker();

  @override
  void initState() {
    super.initState();
    _api = widget.apiServiceFactory != null
        ? widget.apiServiceFactory!(widget.connection)
        : ApiService(widget.connection);
    _fetchStatus();
    _pollTimer =
        Timer.periodic(const Duration(seconds: 3), (_) => _fetchStatus());
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    super.dispose();
  }

  Future<void> _fetchStatus() async {
    if (_tokenExpired) return;
    try {
      final status = await _api.getStatus();
      if (!mounted) return;
      setState(() {
        _status = status;
        _loadingStatus = false;
        _statusError = null;
      });
      if (status.sessionEnded) _pollTimer?.cancel();
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _statusError = e.message;
        _loadingStatus = false;
        if (e.statusCode == 401) _tokenExpired = true;
      });
      if (e.statusCode == 401) _pollTimer?.cancel();
    } catch (_) {
      if (!mounted) return;
      // Network blip — keep trying, just show stale state
      setState(() {
        _statusError ??= 'Trying to reconnect…';
        _loadingStatus = false;
      });
    }
  }

  Future<void> _takePhoto() async {
    XFile? photo;
    try {
      photo = await _picker.pickImage(
        source: ImageSource.camera,
        imageQuality: 85,
        maxWidth: 1920,
      );
    } catch (_) {
      if (mounted) {
        setState(() {
          _uploadSuccess = false;
          _uploadMessage = 'Camera access denied';
        });
      }
      return;
    }

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
          _pollTimer?.cancel();
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

    if (_status!.activeItemId == null) {
      return _StatusCard(
        color: theme.colorScheme.surfaceContainerHighest,
        onColor: theme.colorScheme.onSurface,
        icon: Icons.hourglass_empty_rounded,
        title: 'Waiting for item…',
        subtitle: 'Scan an item in the stocker app first',
      );
    }

    return _StatusCard(
      color: theme.colorScheme.primaryContainer,
      onColor: theme.colorScheme.onPrimaryContainer,
      icon: Icons.check_circle_outline_rounded,
      title: 'Ready for photo',
      subtitle:
          'Item: ${_status!.activeItemId!.substring(0, 8)}…',
    );
  }

  Widget _buildPhotoButton() {
    final theme = Theme.of(context);
    final canPhoto = !_uploading &&
        !_tokenExpired &&
        _status != null &&
        !_status!.sessionEnded &&
        _status!.activeItemId != null;

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
