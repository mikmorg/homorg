import 'dart:async';

import 'package:flutter/material.dart';

import '../models/item.dart';
import '../services/homorg_api.dart';

class ContainerPickerSheet extends StatefulWidget {
  final HomorgApi api;
  final ScrollController scrollController;

  const ContainerPickerSheet({
    required this.api,
    required this.scrollController,
  });

  @override
  State<ContainerPickerSheet> createState() => _ContainerPickerSheetState();
}

class _ContainerPickerSheetState extends State<ContainerPickerSheet> {
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
                        PickerResult(id: c.id, name: c.displayName),
                      ),
                    );
                  },
                ),
        ),
      ],
    );
  }
}

class PickerResult {
  final String id;
  final String name;
  const PickerResult({required this.id, required this.name});
}
