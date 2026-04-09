import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../models/camera_models.dart';
import 'qr_scan_screen.dart';
import 'session_screen.dart';

const _kRecentUrlsKey = 'recent_urls';
const _kMaxRecent = 5;

class ConnectScreen extends StatefulWidget {
  const ConnectScreen({super.key});

  @override
  State<ConnectScreen> createState() => _ConnectScreenState();
}

class _ConnectScreenState extends State<ConnectScreen> {
  final _urlController = TextEditingController();
  List<String> _recentUrls = [];
  String? _validationError;

  @override
  void initState() {
    super.initState();
    _urlController.addListener(_onUrlChanged);
    _loadRecent();
  }

  @override
  void dispose() {
    _urlController.dispose();
    super.dispose();
  }

  void _onUrlChanged() {
    if (_validationError != null) setState(() => _validationError = null);
  }

  Future<void> _loadRecent() async {
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_kRecentUrlsKey);
    if (raw != null && mounted) {
      setState(() => _recentUrls = (jsonDecode(raw) as List).cast<String>());
    }
  }

  Future<void> _saveRecent(String url) async {
    final prefs = await SharedPreferences.getInstance();
    final updated = [url, ..._recentUrls.where((u) => u != url)]
        .take(_kMaxRecent)
        .toList();
    await prefs.setString(_kRecentUrlsKey, jsonEncode(updated));
    if (mounted) setState(() => _recentUrls = updated);
  }

  void _connect(String raw) {
    final connection = CameraConnection.tryParse(raw);
    if (connection == null) {
      setState(() => _validationError =
          'Paste the full upload URL from the stocker page camera panel');
      return;
    }
    _saveRecent(raw.trim());
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => SessionScreen(connection: connection),
      ),
    );
  }

  Future<void> _scanQr() async {
    final url = await Navigator.push<String>(
      context,
      MaterialPageRoute(builder: (_) => const QrScanScreen()),
    );
    if (url != null && mounted) {
      _urlController.text = url;
      _connect(url);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const SizedBox(height: 32),
              Icon(
                Icons.camera_alt_rounded,
                size: 56,
                color: theme.colorScheme.primary,
              ),
              const SizedBox(height: 12),
              Text(
                'Homorg Camera',
                textAlign: TextAlign.center,
                style: theme.textTheme.headlineMedium
                    ?.copyWith(fontWeight: FontWeight.bold),
              ),
              const SizedBox(height: 4),
              Text(
                'Attach photos to stocker sessions',
                textAlign: TextAlign.center,
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: theme.colorScheme.onSurface.withValues(alpha: 0.6),
                ),
              ),
              const SizedBox(height: 48),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(
                    child: TextField(
                      controller: _urlController,
                      decoration: InputDecoration(
                        labelText: 'Upload URL',
                        hintText:
                            'http://192.168.x.x:8080/api/v1/stocker/camera/.../upload',
                        errorText: _validationError,
                        border: const OutlineInputBorder(),
                        prefixIcon: const Icon(Icons.link),
                      ),
                      keyboardType: TextInputType.url,
                      autocorrect: false,
                      onSubmitted: _connect,
                    ),
                  ),
                  const SizedBox(width: 8),
                  IconButton.filled(
                    onPressed: _scanQr,
                    icon: const Icon(Icons.qr_code_scanner),
                    iconSize: 28,
                    style: IconButton.styleFrom(
                      minimumSize: const Size(56, 56),
                    ),
                    tooltip: 'Scan QR code',
                  ),
                ],
              ),
              const SizedBox(height: 16),
              ListenableBuilder(
                listenable: _urlController,
                builder: (_, __) {
                  final canConnect = _urlController.text.trim().isNotEmpty;
                  return FilledButton.icon(
                  onPressed: canConnect ? () => _connect(_urlController.text) : null,
                  icon: const Icon(Icons.link),
                  label: const Text('Connect'),
                );
                },
              ),
              if (_recentUrls.isNotEmpty) ...[
                const SizedBox(height: 32),
                Text('Recent', style: theme.textTheme.labelLarge),
                const SizedBox(height: 4),
                ..._recentUrls.map(
                  (url) => ListTile(
                    leading: const Icon(Icons.history, size: 18),
                    title: Text(
                      url,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.bodySmall,
                    ),
                    trailing: const Icon(Icons.arrow_forward_ios, size: 12),
                    contentPadding: EdgeInsets.zero,
                    dense: true,
                    onTap: () {
                      _urlController.text = url;
                      _connect(url);
                    },
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
