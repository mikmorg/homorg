import 'package:flutter/material.dart';

import '../models/item.dart';
import '../services/homorg_api.dart';
import 'create_item_screen.dart';
import 'item_detail_screen.dart';

const _rootContainerId = '00000000-0000-0000-0000-000000000001';

class BrowseScreen extends StatefulWidget {
  final HomorgApi api;

  const BrowseScreen({super.key, required this.api});

  @override
  State<BrowseScreen> createState() => _BrowseScreenState();
}

class _BrowseScreenState extends State<BrowseScreen> {
  String _containerId = _rootContainerId;
  List<AncestorEntry> _breadcrumb = [];
  List<ItemSummary> _children = [];
  Item? _currentItem;
  bool _loading = true;
  String? _error;

  // Pagination
  String? _childrenCursor;
  bool _hasMoreChildren = false;
  bool _loadingMoreChildren = false;

  @override
  void initState() {
    super.initState();
    _navigate(_rootContainerId);
  }

  Future<void> _navigate(String containerId) async {
    setState(() {
      _containerId = containerId;
      _loading = true;
      _error = null;
      _childrenCursor = null;
      _hasMoreChildren = false;
      _loadingMoreChildren = false;
    });

    try {
      final futures = await Future.wait([
        widget.api.getAncestors(containerId),
        widget.api.getChildren(containerId),
        widget.api.getItem(containerId),
      ]);
      if (!mounted) return;
      final children = futures[1] as List<ItemSummary>;
      final currentItem = futures[2] as Item;
      setState(() {
        _breadcrumb = futures[0] as List<AncestorEntry>;
        _children = children;
        _currentItem = currentItem;
        _hasMoreChildren = children.length == 50;
        _childrenCursor = children.isNotEmpty ? children.last.id : null;
        _loading = false;
      });
    } on ApiError catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e.message;
        _loading = false;
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _error = 'Failed to load';
        _loading = false;
      });
    }
  }

  Future<void> _openItem(ItemSummary item) async {
    final result = await Navigator.of(context).push<String>(
      MaterialPageRoute(
        builder: (_) => ItemDetailScreen(
          api: widget.api,
          itemId: item.id,
        ),
      ),
    );
    // Refresh if something changed
    if (result == 'deleted' || result == 'updated') {
      _navigate(_containerId);
    }
  }

  Future<void> _loadMoreChildren() async {
    if (_loadingMoreChildren || _childrenCursor == null) return;
    setState(() => _loadingMoreChildren = true);
    try {
      final more = await widget.api
          .getChildren(_containerId, cursor: _childrenCursor);
      if (mounted) {
        setState(() {
          _children = [..._children, ...more];
          _hasMoreChildren = more.length == 50;
          _childrenCursor = more.isNotEmpty ? more.last.id : null;
          _loadingMoreChildren = false;
        });
      }
    } catch (_) {
      if (mounted) {
        setState(() => _loadingMoreChildren = false);
      }
    }
  }

  Future<void> _createItem() async {
    final itemId = await Navigator.of(context).push<String>(
      MaterialPageRoute(
        builder: (_) => CreateItemScreen(
          api: widget.api,
          initialContainerId: _containerId,
        ),
      ),
    );
    if (itemId != null && mounted) {
      // Show the newly created item, then refresh browse on return
      await Navigator.of(context).push<String>(
        MaterialPageRoute(
          builder: (_) => ItemDetailScreen(
            api: widget.api,
            itemId: itemId,
          ),
        ),
      );
      if (mounted) {
        _navigate(_containerId);
      }
    }
  }

  // ── Build ──────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Column(
        children: [
          _buildBreadcrumb(),
          const Divider(height: 1),
          Expanded(child: _buildBody()),
        ],
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: _createItem,
        tooltip: 'Create item here',
        child: const Icon(Icons.add),
      ),
    );
  }

  Widget _buildBreadcrumb() {
    // Build crumb list: Root + ancestors (skip root) + current
    final crumbs = <_Crumb>[
      const _Crumb('Root', _rootContainerId),
    ];
    for (final a in _breadcrumb) {
      if (a.id != _rootContainerId) {
        crumbs.add(_Crumb(a.name ?? '?', a.id));
      }
    }
    if (_currentItem != null) {
      crumbs.add(_Crumb(_currentItem!.displayName, _currentItem!.id));
    }

    return Container(
      color: Theme.of(context).colorScheme.surfaceContainerLow,
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
      child: SizedBox(
        height: 32,
        child: ListView.separated(
          scrollDirection: Axis.horizontal,
          itemCount: crumbs.length,
          separatorBuilder: (_, __) => const Padding(
            padding: EdgeInsets.symmetric(horizontal: 2),
            child: Icon(Icons.chevron_right, size: 18),
          ),
          itemBuilder: (context, i) {
            final crumb = crumbs[i];
            final isCurrent = crumb.id == _containerId;
            return ActionChip(
              label: Text(
                crumb.label,
                style: TextStyle(
                  fontWeight: isCurrent ? FontWeight.bold : FontWeight.normal,
                  fontSize: 13,
                ),
              ),
              onPressed: isCurrent ? null : () => _navigate(crumb.id),
              visualDensity: VisualDensity.compact,
              padding: EdgeInsets.zero,
            );
          },
        ),
      ),
    );
  }

  Widget _buildBody() {
    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }

    if (_error != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(_error!, style: TextStyle(color: Theme.of(context).colorScheme.error)),
            const SizedBox(height: 12),
            FilledButton.tonal(
              onPressed: () => _navigate(_containerId),
              child: const Text('Retry'),
            ),
          ],
        ),
      );
    }

    if (_children.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.folder_open, size: 64, color: Theme.of(context).colorScheme.outline),
            const SizedBox(height: 12),
            Text('Empty container', style: Theme.of(context).textTheme.bodyLarge),
            const SizedBox(height: 4),
            Text('Tap + to add an item', style: Theme.of(context).textTheme.bodySmall),
          ],
        ),
      );
    }

    // Sort: containers first, then alphabetically
    final sorted = List<ItemSummary>.from(_children)
      ..sort((a, b) {
        if (a.isContainer != b.isContainer) {
          return a.isContainer ? -1 : 1;
        }
        return a.displayName.toLowerCase().compareTo(b.displayName.toLowerCase());
      });

    return RefreshIndicator(
      onRefresh: () => _navigate(_containerId),
      child: ListView(
        children: [
          ...sorted.map((item) => _buildChildTile(item)),
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
      ),
    );
  }

  Widget _buildChildTile(ItemSummary item) {
    final isContainer = item.isContainer;

    return ListTile(
      leading: Icon(
        isContainer ? Icons.folder : Icons.inventory_2_outlined,
        color: isContainer
            ? Theme.of(context).colorScheme.primary
            : Theme.of(context).colorScheme.onSurfaceVariant,
      ),
      title: Text(
        item.displayName,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: _buildSubtitle(item),
      trailing: isContainer
          ? const Icon(Icons.chevron_right)
          : null,
      onTap: () {
        if (isContainer) {
          _navigate(item.id);
        } else {
          _openItem(item);
        }
      },
    );
  }

  Widget? _buildSubtitle(ItemSummary item) {
    final parts = <String>[];
    if (item.category != null) parts.add(item.category!);
    if (item.condition != null) parts.add(item.condition!);
    if (parts.isEmpty) return null;
    return Text(
      parts.join(' · '),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
    );
  }
}

class _Crumb {
  final String label;
  final String id;
  const _Crumb(this.label, this.id);
}
