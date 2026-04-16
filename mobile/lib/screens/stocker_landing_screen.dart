import 'package:flutter/material.dart';

import '../models/session.dart';
import '../services/homorg_api.dart';
import 'connect_screen.dart';
import 'direct_stocker_screen.dart';
import 'session_detail_screen.dart';

/// Landing screen for the Stocker tab. Shows active sessions (direct mode)
/// and a "Connect as Camera" entry point for proxy mode.
class StockerLandingScreen extends StatefulWidget {
  final HomorgApi api;

  const StockerLandingScreen({super.key, required this.api});

  @override
  State<StockerLandingScreen> createState() => _StockerLandingScreenState();
}

class _StockerLandingScreenState extends State<StockerLandingScreen> {
  List<ScanSession>? _sessions;
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _loadSessions();
  }

  Future<void> _loadSessions() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final sessions = await widget.api.listSessions(limit: 50);
      if (!mounted) return;
      setState(() {
        _sessions = sessions;
        _loading = false;
      });
    } on ApiError catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e.message;
        _loading = false;
      });
    }
  }

  Future<void> _createSession() async {
    final notes = await _showNewSessionDialog();
    if (notes == null || !mounted) return;

    try {
      final session = await widget.api.createSession(
        notes: notes.isEmpty ? null : notes,
        deviceId: 'homorg-mobile',
      );
      if (!mounted) return;
      _openDirectSession(session);
    } on ApiError catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Failed to create session: ${e.message}')),
      );
    }
  }

  Future<String?> _showNewSessionDialog() {
    final controller = TextEditingController();
    return showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('New Session'),
        content: TextField(
          controller: controller,
          decoration: const InputDecoration(
            labelText: 'Name (optional)',
            hintText: 'e.g. Kitchen shelves',
            border: OutlineInputBorder(),
          ),
          autofocus: true,
          textCapitalization: TextCapitalization.sentences,
          onSubmitted: (v) => Navigator.pop(ctx, v),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, controller.text),
            child: const Text('Start'),
          ),
        ],
      ),
    );
  }

  void _openDirectSession(ScanSession session) {
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => DirectStockerScreen(
          api: widget.api,
          session: session,
        ),
      ),
    ).then((_) {
      if (mounted) _loadSessions();
    });
  }

  void _openProxyMode() {
    Navigator.push(
      context,
      MaterialPageRoute(builder: (_) => const ConnectScreen()),
    ).then((_) {
      if (mounted) _loadSessions();
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return SafeArea(
      child: Column(
        children: [
          // Action buttons
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Row(
              children: [
                Expanded(
                  child: FilledButton.icon(
                    onPressed: _createSession,
                    icon: const Icon(Icons.add),
                    label: const Text('New Session'),
                  ),
                ),
                const SizedBox(width: 12),
                OutlinedButton.icon(
                  onPressed: _openProxyMode,
                  icon: const Icon(Icons.linked_camera),
                  label: const Text('Camera Proxy'),
                ),
              ],
            ),
          ),
          const Divider(height: 1),
          // Session list
          Expanded(child: _buildBody(theme)),
        ],
      ),
    );
  }

  Widget _buildBody(ThemeData theme) {
    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_error != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.error_outline, size: 48,
                color: theme.colorScheme.error),
            const SizedBox(height: 12),
            Text(_error!, style: theme.textTheme.bodyMedium),
            const SizedBox(height: 12),
            OutlinedButton.icon(
              onPressed: _loadSessions,
              icon: const Icon(Icons.refresh),
              label: const Text('Retry'),
            ),
          ],
        ),
      );
    }
    if (_sessions == null || _sessions!.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.inventory_2_outlined, size: 56,
                color: theme.colorScheme.onSurface.withValues(alpha: 0.3)),
            const SizedBox(height: 12),
            Text(
              'No sessions yet',
              style: theme.textTheme.bodyLarge?.copyWith(
                color: theme.colorScheme.onSurface.withValues(alpha: 0.5),
              ),
            ),
            const SizedBox(height: 4),
            Text(
              'Start a new session to begin stocking',
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurface.withValues(alpha: 0.4),
              ),
            ),
          ],
        ),
      );
    }

    // Separate active and ended sessions
    final active = _sessions!.where((s) => s.isActive).toList();
    final ended = _sessions!.where((s) => !s.isActive).toList();

    return RefreshIndicator(
      onRefresh: _loadSessions,
      child: ListView(
        padding: const EdgeInsets.symmetric(vertical: 8),
        children: [
          if (active.isNotEmpty) ...[
            _sectionHeader(theme, 'Active'),
            ...active.map((s) => _sessionTile(theme, s)),
          ],
          if (ended.isNotEmpty) ...[
            _sectionHeader(theme, 'Recent'),
            ...ended.map((s) => _sessionTile(theme, s)),
          ],
        ],
      ),
    );
  }

  Widget _sectionHeader(ThemeData theme, String title) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 4),
      child: Text(
        title,
        style: theme.textTheme.labelLarge?.copyWith(
          color: theme.colorScheme.primary,
        ),
      ),
    );
  }

  Widget _sessionTile(ThemeData theme, ScanSession session) {
    final isActive = session.isActive;
    return ListTile(
      leading: CircleAvatar(
        backgroundColor: isActive
            ? theme.colorScheme.primaryContainer
            : theme.colorScheme.surfaceContainerHighest,
        child: Icon(
          isActive ? Icons.play_arrow : Icons.stop,
          color: isActive
              ? theme.colorScheme.onPrimaryContainer
              : theme.colorScheme.onSurface.withValues(alpha: 0.5),
          size: 20,
        ),
      ),
      title: Text(
        session.displayName,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: Text(
        '${session.totalItems} items  •  '
        '${session.itemsCreated} created  •  '
        '${session.itemsMoved} moved',
        style: theme.textTheme.bodySmall,
      ),
      trailing: isActive
          ? const Icon(Icons.arrow_forward_ios, size: 14)
          : const Icon(Icons.chevron_right, size: 14),
      onTap: isActive
          ? () => _openDirectSession(session)
          : () => Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (_) => SessionDetailScreen(
                    api: widget.api,
                    session: session,
                  ),
                ),
              ),
    );
  }
}
