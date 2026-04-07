import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// Prompts the user to paste the camera upload URL from clipboard.
/// QR code scanning can be added later via mobile_scanner once disk space
/// is available on the build machine.
class QrScanScreen extends StatefulWidget {
  const QrScanScreen({super.key});

  @override
  State<QrScanScreen> createState() => _QrScanScreenState();
}

class _QrScanScreenState extends State<QrScanScreen> {
  String? _clipboardText;
  bool _checked = false;

  @override
  void initState() {
    super.initState();
    _readClipboard();
  }

  Future<void> _readClipboard() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    if (mounted) {
      setState(() {
        _clipboardText = data?.text;
        _checked = true;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final hasUrl = _clipboardText != null &&
        _clipboardText!.contains('/camera/');

    return Scaffold(
      appBar: AppBar(title: const Text('Paste URL')),
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const SizedBox(height: 16),
              Icon(Icons.content_paste_rounded,
                  size: 56, color: theme.colorScheme.primary),
              const SizedBox(height: 16),
              Text(
                'Copy the upload URL from the stocker page,\nthen tap the button below.',
                textAlign: TextAlign.center,
                style: theme.textTheme.bodyMedium,
              ),
              const SizedBox(height: 32),
              if (!_checked)
                const Center(child: CircularProgressIndicator())
              else if (hasUrl) ...[
                Card(
                  color: theme.colorScheme.primaryContainer,
                  child: Padding(
                    padding: const EdgeInsets.all(12),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text('URL detected in clipboard:',
                            style: TextStyle(
                                color: theme.colorScheme.onPrimaryContainer,
                                fontSize: 12)),
                        const SizedBox(height: 4),
                        Text(
                          _clipboardText!,
                          style: TextStyle(
                            color: theme.colorScheme.onPrimaryContainer,
                            fontSize: 11,
                            fontFamily: 'monospace',
                          ),
                          maxLines: 3,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 16),
                FilledButton.icon(
                  onPressed: () => Navigator.pop(context, _clipboardText),
                  icon: const Icon(Icons.check),
                  label: const Text('Use this URL'),
                ),
              ] else ...[
                FilledButton.icon(
                  onPressed: () async {
                    final messenger = ScaffoldMessenger.of(context);
                    await _readClipboard();
                    if (!mounted) return;
                    if (_clipboardText == null || !_clipboardText!.contains('/camera/')) {
                      messenger.showSnackBar(
                        const SnackBar(content: Text('No camera URL found in clipboard')),
                      );
                    }
                  },
                  icon: const Icon(Icons.content_paste),
                  label: const Text('Check clipboard'),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
