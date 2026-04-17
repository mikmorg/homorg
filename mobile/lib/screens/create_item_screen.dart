import 'dart:async';

import 'package:flutter/material.dart';

import '../models/item.dart';
import '../services/homorg_api.dart';

const _conditions = ['new', 'like_new', 'good', 'fair', 'poor', 'broken'];

class CreateItemScreen extends StatefulWidget {
  final HomorgApi api;
  final String? initialContainerId;

  const CreateItemScreen({
    super.key,
    required this.api,
    this.initialContainerId,
  });

  @override
  State<CreateItemScreen> createState() => _CreateItemScreenState();
}

class _CreateItemScreenState extends State<CreateItemScreen> {
  final _formKey = GlobalKey<FormState>();

  // Container selection
  String? _containerId;
  String _containerName = '';
  bool _loadingContainer = false;

  // Fields
  final _nameCtrl = TextEditingController();
  final _descCtrl = TextEditingController();
  final _categoryCtrl = TextEditingController();
  String? _condition;
  final _tagCtrl = TextEditingController();
  final List<String> _tags = [];
  bool _isContainer = false;
  String? _containerTypeId;
  bool _isFungible = false;
  final _quantityCtrl = TextEditingController();
  final _unitCtrl = TextEditingController();

  // Valuation
  final _acqDateCtrl = TextEditingController();
  final _acqCostCtrl = TextEditingController();
  final _valueCtrl = TextEditingController();
  final _warrantyCtrl = TextEditingController();

  bool _saving = false;

  // Taxonomy
  List<String> _knownCategories = [];
  List<String> _knownTags = [];
  List<ContainerType> _containerTypes = [];

  @override
  void initState() {
    super.initState();
    if (widget.initialContainerId != null) {
      _containerId = widget.initialContainerId;
      _loadContainerName(widget.initialContainerId!);
    }
    _fetchTaxonomy();
  }

  Future<void> _fetchTaxonomy() async {
    try {
      final results = await Future.wait([
        widget.api.listCategories(),
        widget.api.listTags(),
        widget.api.listContainerTypes(),
      ]);
      if (mounted) {
        setState(() {
          _knownCategories = results[0];
          _knownTags = results[1];
          _containerTypes = results[2];
        });
      }
    } catch (_) {}
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _descCtrl.dispose();
    _categoryCtrl.dispose();
    _tagCtrl.dispose();
    _quantityCtrl.dispose();
    _unitCtrl.dispose();
    _acqDateCtrl.dispose();
    _acqCostCtrl.dispose();
    _valueCtrl.dispose();
    _warrantyCtrl.dispose();
    super.dispose();
  }

  Future<void> _pickDate(TextEditingController ctrl) async {
    final initial = DateTime.tryParse(ctrl.text) ?? DateTime.now();
    final picked = await showDatePicker(
      context: context,
      initialDate: initial,
      firstDate: DateTime(2000),
      lastDate: DateTime(2100),
    );
    if (picked != null) {
      ctrl.text =
          '${picked.year}-${picked.month.toString().padLeft(2, '0')}-${picked.day.toString().padLeft(2, '0')}';
    }
  }

  Future<void> _loadContainerName(String id) async {
    setState(() => _loadingContainer = true);
    try {
      final item = await widget.api.getItem(id);
      if (mounted) {
        setState(() {
          _containerName = item.displayName;
          _loadingContainer = false;
        });
      }
    } catch (_) {
      if (mounted) {
        setState(() {
          _containerName = id;
          _loadingContainer = false;
        });
      }
    }
  }

  void _addTag() {
    final tag = _tagCtrl.text.trim();
    if (tag.isNotEmpty && !_tags.contains(tag)) {
      setState(() {
        _tags.add(tag);
        _tagCtrl.clear();
      });
    }
  }

  Future<void> _pickContainer() async {
    final result = await showModalBottomSheet<_PickedContainer>(
      context: context,
      isScrollControlled: true,
      builder: (_) => _ContainerPickerSheet(api: widget.api),
    );
    if (result != null) {
      setState(() {
        _containerId = result.id;
        _containerName = result.name;
      });
    }
  }

  Future<void> _save() async {
    if (!_formKey.currentState!.validate()) return;
    if (_containerId == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Select a parent container')),
      );
      return;
    }

    setState(() => _saving = true);

    final body = <String, dynamic>{
      'parent_id': _containerId,
      'name': _nameCtrl.text.trim(),
      'is_container': _isContainer,
    };

    final desc = _descCtrl.text.trim();
    if (desc.isNotEmpty) body['description'] = desc;
    final cat = _categoryCtrl.text.trim();
    if (cat.isNotEmpty) body['category'] = cat;
    if (_condition != null) body['condition'] = _condition;
    if (_tags.isNotEmpty) body['tags'] = _tags;
    if (_containerTypeId != null) body['container_type_id'] = _containerTypeId;
    if (_isFungible) {
      body['is_fungible'] = true;
      final qty = int.tryParse(_quantityCtrl.text.trim());
      if (qty != null) body['fungible_quantity'] = qty;
      final unit = _unitCtrl.text.trim();
      if (unit.isNotEmpty) body['fungible_unit'] = unit;
    }

    // Valuation
    final acqDate = _acqDateCtrl.text.trim();
    if (acqDate.isNotEmpty) body['acquisition_date'] = acqDate;
    final acqCost = double.tryParse(_acqCostCtrl.text.trim());
    if (acqCost != null) body['acquisition_cost'] = acqCost;
    final curVal = double.tryParse(_valueCtrl.text.trim());
    if (curVal != null) body['current_value'] = curVal;
    final warranty = _warrantyCtrl.text.trim();
    if (warranty.isNotEmpty) body['warranty_expiry'] = warranty;

    try {
      final item = await widget.api.createItem(body);
      if (!mounted) return;
      // Pop back with the new item's ID so the caller can navigate to it
      Navigator.of(context).pop(item.id);
    } on ApiError catch (e) {
      if (!mounted) return;
      setState(() => _saving = false);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(e.message)),
      );
    } catch (_) {
      if (!mounted) return;
      setState(() => _saving = false);
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Failed to create item')),
      );
    }
  }

  // ── Build ──────────────────────────────────────────────────────────

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('New Item'),
        actions: [
          TextButton(
            onPressed: _saving ? null : _save,
            child: _saving
                ? const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Text('Save'),
          ),
        ],
      ),
      body: Form(
        key: _formKey,
        child: ListView(
          padding: const EdgeInsets.all(16),
          children: [
            // Parent container
            _buildContainerPicker(),
            const SizedBox(height: 16),

            // Name
            TextFormField(
              controller: _nameCtrl,
              decoration: const InputDecoration(
                labelText: 'Name *',
                border: OutlineInputBorder(),
              ),
              textCapitalization: TextCapitalization.sentences,
              validator: (v) =>
                  (v == null || v.trim().isEmpty) ? 'Name is required' : null,
            ),
            const SizedBox(height: 16),

            // Description
            TextFormField(
              controller: _descCtrl,
              decoration: const InputDecoration(
                labelText: 'Description',
                border: OutlineInputBorder(),
              ),
              textCapitalization: TextCapitalization.sentences,
              maxLines: 3,
              minLines: 1,
            ),
            const SizedBox(height: 16),

            // Category (autocomplete from known categories)
            Autocomplete<String>(
              optionsBuilder: (textEditingValue) {
                if (_knownCategories.isEmpty) return const Iterable.empty();
                if (textEditingValue.text.isEmpty) return _knownCategories;
                final q = textEditingValue.text.toLowerCase();
                return _knownCategories
                    .where((c) => c.toLowerCase().contains(q));
              },
              onSelected: (value) => _categoryCtrl.text = value,
              fieldViewBuilder: (ctx, ctrl, fn, onSubmit) {
                ctrl.addListener(() => _categoryCtrl.text = ctrl.text);
                return TextField(
                  controller: ctrl,
                  focusNode: fn,
                  decoration: const InputDecoration(
                    labelText: 'Category',
                    border: OutlineInputBorder(),
                  ),
                  textCapitalization: TextCapitalization.sentences,
                );
              },
            ),
            const SizedBox(height: 16),

            // Condition
            DropdownButtonFormField<String?>(
              initialValue: _condition,
              decoration: const InputDecoration(
                labelText: 'Condition',
                border: OutlineInputBorder(),
              ),
              items: [
                const DropdownMenuItem(value: null, child: Text('None')),
                ..._conditions.map((c) => DropdownMenuItem(
                      value: c,
                      child: Text(c.replaceAll('_', ' ')),
                    )),
              ],
              onChanged: (v) => setState(() => _condition = v),
            ),
            const SizedBox(height: 16),

            // Tags
            _buildTagsSection(),
            // Tag suggestions from taxonomy
            if (_knownTags.where((t) => !_tags.contains(t)).isNotEmpty) ...[
              const SizedBox(height: 8),
              Wrap(
                spacing: 6,
                runSpacing: 4,
                children: _knownTags
                    .where((t) => !_tags.contains(t))
                    .map((t) => ActionChip(
                          label: Text(t, style: const TextStyle(fontSize: 12)),
                          visualDensity: VisualDensity.compact,
                          onPressed: () => setState(() => _tags.add(t)),
                        ))
                    .toList(),
              ),
            ],
            const SizedBox(height: 16),

            // Is Container toggle
            SwitchListTile(
              title: const Text('Is a container'),
              subtitle: const Text('Can hold other items'),
              value: _isContainer,
              onChanged: (v) => setState(() => _isContainer = v),
              contentPadding: EdgeInsets.zero,
            ),

            if (_isContainer) ...[
              const SizedBox(height: 12),
              DropdownButtonFormField<String?>(
                value: _containerTypeId,
                decoration: const InputDecoration(
                  labelText: 'Container Type',
                  border: OutlineInputBorder(),
                ),
                items: [
                  const DropdownMenuItem(value: null, child: Text('None')),
                  ..._containerTypes.map((ct) => DropdownMenuItem(
                        value: ct.id,
                        child: Text(ct.name),
                      )),
                ],
                onChanged: (v) => setState(() => _containerTypeId = v),
              ),
            ],

            // Fungible toggle
            SwitchListTile(
              title: const Text('Fungible / consumable'),
              subtitle: const Text('Track by quantity instead of individually'),
              value: _isFungible,
              onChanged: (v) => setState(() => _isFungible = v),
              contentPadding: EdgeInsets.zero,
            ),

            if (_isFungible) ...[
              const SizedBox(height: 8),
              Row(
                children: [
                  Expanded(
                    child: TextFormField(
                      controller: _quantityCtrl,
                      decoration: const InputDecoration(
                        labelText: 'Quantity',
                        border: OutlineInputBorder(),
                      ),
                      keyboardType: TextInputType.number,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: TextFormField(
                      controller: _unitCtrl,
                      decoration: const InputDecoration(
                        labelText: 'Unit',
                        hintText: 'e.g. ml, pcs',
                        border: OutlineInputBorder(),
                      ),
                    ),
                  ),
                ],
              ),
            ],

            // ── Valuation section ──
            const SizedBox(height: 24),
            Text('Valuation',
                style: Theme.of(context).textTheme.titleSmall),
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _acqDateCtrl,
                    decoration: InputDecoration(
                      labelText: 'Acquisition date',
                      border: const OutlineInputBorder(),
                      suffixIcon: IconButton(
                        icon: const Icon(Icons.calendar_today, size: 18),
                        onPressed: () => _pickDate(_acqDateCtrl),
                      ),
                    ),
                    readOnly: true,
                    onTap: () => _pickDate(_acqDateCtrl),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: TextField(
                    controller: _acqCostCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Cost',
                      border: OutlineInputBorder(),
                    ),
                    keyboardType:
                        const TextInputType.numberWithOptions(decimal: true),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _valueCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Current value',
                      border: OutlineInputBorder(),
                    ),
                    keyboardType:
                        const TextInputType.numberWithOptions(decimal: true),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: TextField(
                    controller: _warrantyCtrl,
                    decoration: InputDecoration(
                      labelText: 'Warranty expiry',
                      border: const OutlineInputBorder(),
                      suffixIcon: IconButton(
                        icon: const Icon(Icons.calendar_today, size: 18),
                        onPressed: () => _pickDate(_warrantyCtrl),
                      ),
                    ),
                    readOnly: true,
                    onTap: () => _pickDate(_warrantyCtrl),
                  ),
                ),
              ],
            ),

            const SizedBox(height: 80), // FAB clearance
          ],
        ),
      ),
    );
  }

  Widget _buildContainerPicker() {
    return InkWell(
      onTap: _pickContainer,
      borderRadius: BorderRadius.circular(12),
      child: InputDecorator(
        decoration: const InputDecoration(
          labelText: 'Parent Container *',
          border: OutlineInputBorder(),
        ),
        child: Row(
          children: [
            const Icon(Icons.folder, size: 20),
            const SizedBox(width: 8),
            Expanded(
              child: _loadingContainer
                  ? const SizedBox(
                      height: 16,
                      width: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : Text(
                      _containerId != null ? _containerName : 'Select…',
                      style: TextStyle(
                        color: _containerId != null
                            ? null
                            : Theme.of(context).colorScheme.onSurfaceVariant,
                      ),
                    ),
            ),
            const Icon(Icons.chevron_right, size: 20),
          ],
        ),
      ),
    );
  }

  Widget _buildTagsSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (_tags.isNotEmpty)
          Wrap(
            spacing: 6,
            runSpacing: 4,
            children: _tags
                .map((t) => Chip(
                      label: Text(t),
                      onDeleted: () => setState(() => _tags.remove(t)),
                      visualDensity: VisualDensity.compact,
                    ))
                .toList(),
          ),
        if (_tags.isNotEmpty) const SizedBox(height: 8),
        Row(
          children: [
            Expanded(
              child: TextFormField(
                controller: _tagCtrl,
                decoration: const InputDecoration(
                  labelText: 'Add tag',
                  border: OutlineInputBorder(),
                ),
                onFieldSubmitted: (_) => _addTag(),
              ),
            ),
            const SizedBox(width: 8),
            IconButton.filled(
              onPressed: _addTag,
              icon: const Icon(Icons.add),
            ),
          ],
        ),
      ],
    );
  }
}

// ── Container picker sheet ──────────────────────────────────────────

class _PickedContainer {
  final String id;
  final String name;
  const _PickedContainer(this.id, this.name);
}

class _ContainerPickerSheet extends StatefulWidget {
  final HomorgApi api;
  const _ContainerPickerSheet({required this.api});

  @override
  State<_ContainerPickerSheet> createState() => _ContainerPickerSheetState();
}

class _ContainerPickerSheetState extends State<_ContainerPickerSheet> {
  final _searchCtrl = TextEditingController();
  Timer? _debounce;
  List<ItemSummary> _results = [];
  bool _searching = false;

  @override
  void initState() {
    super.initState();
    _search('');
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    _debounce?.cancel();
    super.dispose();
  }

  void _onSearch(String query) {
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 300), () => _search(query));
  }

  Future<void> _search(String query) async {
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
    return DraggableScrollableSheet(
      initialChildSize: 0.6,
      minChildSize: 0.3,
      maxChildSize: 0.9,
      expand: false,
      builder: (context, scrollCtrl) => Column(
        children: [
          const SizedBox(height: 8),
          Container(
            width: 32,
            height: 4,
            decoration: BoxDecoration(
              color: Theme.of(context).colorScheme.outlineVariant,
              borderRadius: BorderRadius.circular(2),
            ),
          ),
          Padding(
            padding: const EdgeInsets.all(16),
            child: TextField(
              controller: _searchCtrl,
              autofocus: true,
              decoration: InputDecoration(
                hintText: 'Search containers…',
                prefixIcon: const Icon(Icons.search),
                border: const OutlineInputBorder(),
                suffixIcon: _searching
                    ? const Padding(
                        padding: EdgeInsets.all(12),
                        child: SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        ),
                      )
                    : null,
              ),
              onChanged: _onSearch,
            ),
          ),
          Expanded(
            child: ListView.builder(
              controller: scrollCtrl,
              itemCount: _results.length,
              itemBuilder: (context, i) {
                final c = _results[i];
                return ListTile(
                  leading: const Icon(Icons.folder),
                  title: Text(c.displayName),
                  subtitle: c.containerPath != null
                      ? Text(c.containerPath!, maxLines: 1, overflow: TextOverflow.ellipsis)
                      : null,
                  onTap: () => Navigator.pop(
                    context,
                    _PickedContainer(c.id, c.displayName),
                  ),
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}
