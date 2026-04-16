import 'package:flutter/material.dart';

import '../models/session.dart';
import '../services/homorg_api.dart';

String _formatDateTime(String iso) {
  final dt = DateTime.tryParse(iso);
  if (dt == null) return iso;
  final local = dt.toLocal();
  return '${local.year}-${local.month.toString().padLeft(2, '0')}-${local.day.toString().padLeft(2, '0')} '
      '${local.hour.toString().padLeft(2, '0')}:${local.minute.toString().padLeft(2, '0')}';
}

class SessionDetailScreen extends StatefulWidget {
  final HomorgApi api;
  final ScanSession session;

  const SessionDetailScreen({
    super.key,
    required this.api,
    required this.session,
  });

  @override
  State<SessionDetailScreen> createState() => _SessionDetailScreenState();
}

class _SessionDetailScreenState extends State<SessionDetailScreen> {
  late ScanSession _session;
  bool _loading = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _session = widget.session;
    _loadSession();
  }

  Future<void> _loadSession() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final session = await widget.api.getSession(widget.session.id);
      if (mounted) {
        setState(() {
          _session = session;
          _loading = false;
        });
      }
    } on ApiError catch (e) {
      if (mounted) {
        setState(() {
          _error = e.message;
          _loading = false;
        });
      }
    } catch (_) {
      if (mounted) {
        setState(() {
          _loading = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(title: Text(_session.displayName)),
      body: RefreshIndicator(
        onRefresh: _loadSession,
        child: ListView(
          padding: const EdgeInsets.all(16),
          children: [
            if (_error != null)
              Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: Text(
                  _error!,
                  style: TextStyle(color: theme.colorScheme.error),
                ),
              ),
            if (_loading)
              const Padding(
                padding: EdgeInsets.only(bottom: 12),
                child: LinearProgressIndicator(),
              ),

            // Stats card
            Card(
              margin: EdgeInsets.zero,
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: 4),
                child: Column(
                  children: [
                    _StatRow(
                        label: 'Started',
                        value: _formatDateTime(_session.startedAt)),
                    _divider(theme),
                    _StatRow(
                      label: 'Ended',
                      value: _session.endedAt != null
                          ? _formatDateTime(_session.endedAt!)
                          : 'Still active',
                    ),
                    _divider(theme),
                    _StatRow(
                        label: 'Items scanned',
                        value: '${_session.itemsScanned}'),
                    _divider(theme),
                    _StatRow(
                        label: 'Created', value: '${_session.itemsCreated}'),
                    _divider(theme),
                    _StatRow(label: 'Moved', value: '${_session.itemsMoved}'),
                    _divider(theme),
                    _StatRow(
                        label: 'Errors', value: '${_session.itemsErrored}'),
                    _divider(theme),
                    _StatRow(label: 'Device', value: _session.deviceId ?? '—'),
                    _divider(theme),
                    _StatRow(label: 'Notes', value: _session.notes ?? '—'),
                  ],
                ),
              ),
            ),

            const SizedBox(height: 24),

            // Summary
            Card(
              margin: EdgeInsets.zero,
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                  children: [
                    _StatBadge(
                      icon: Icons.add_circle_outline,
                      label: 'Created',
                      value: _session.itemsCreated,
                      color: Colors.green,
                    ),
                    _StatBadge(
                      icon: Icons.drive_file_move_outlined,
                      label: 'Moved',
                      value: _session.itemsMoved,
                      color: Colors.blue,
                    ),
                    _StatBadge(
                      icon: Icons.error_outline,
                      label: 'Errors',
                      value: _session.itemsErrored,
                      color:
                          _session.itemsErrored > 0 ? Colors.red : Colors.grey,
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _divider(ThemeData theme) {
    return Divider(
      height: 1,
      indent: 16,
      endIndent: 16,
      color: theme.colorScheme.outlineVariant.withValues(alpha: 0.4),
    );
  }
}

class _StatRow extends StatelessWidget {
  final String label;
  final String value;

  const _StatRow({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
      child: Row(
        children: [
          SizedBox(
            width: 120,
            child: Text(label,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                )),
          ),
          Expanded(
            child: Text(value, style: theme.textTheme.bodyMedium),
          ),
        ],
      ),
    );
  }
}

class _StatBadge extends StatelessWidget {
  final IconData icon;
  final String label;
  final int value;
  final Color color;

  const _StatBadge({
    required this.icon,
    required this.label,
    required this.value,
    required this.color,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Icon(icon, color: color, size: 28),
        const SizedBox(height: 4),
        Text('$value',
            style: TextStyle(
              fontSize: 20,
              fontWeight: FontWeight.bold,
              color: color,
            )),
        const SizedBox(height: 2),
        Text(label, style: Theme.of(context).textTheme.bodySmall),
      ],
    );
  }
}
