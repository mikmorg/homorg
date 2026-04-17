import 'package:flutter/material.dart';

import '../models/item.dart';
import '../services/homorg_api.dart';

const _conditions = ['new', 'like_new', 'good', 'fair', 'poor', 'broken'];

String _conditionLabel(String c) => c.replaceAll('_', ' ');

class _EditItemPage extends StatefulWidget {
  final HomorgApi api;
  final Item item;

  const _EditItemPage({required this.api, required this.item});

  @override
  State<_EditItemPage> createState() => _EditItemPageState();
}

class _EditItemPageState extends State<_EditItemPage> {
  late final TextEditingController _nameCtrl;
  late final TextEditingController _descCtrl;
  late final TextEditingController _categoryCtrl;
  late String? _condition;
  late final List<String> _tags;
  final _tagCtrl = TextEditingController();
  late bool _isFungible;
  late final TextEditingController _quantityCtrl;
  late final TextEditingController _unitCtrl;

  // Valuation
  late final TextEditingController _acqDateCtrl;
  late final TextEditingController _acqCostCtrl;
  late final TextEditingController _valueCtrl;
  late final TextEditingController _warrantyCtrl;

  // Additional fields
  late bool _isContainer;
  late final TextEditingController _currencyCtrl;
  late final TextEditingController _weightCtrl;
  late String? _containerTypeId;

  bool _saving = false;

  // Taxonomy
  List<String> _knownCategories = [];
  List<String> _knownTags = [];
  List<ContainerType> _containerTypes = [];
  String? _taxonomyError;

  @override
  void initState() {
    super.initState();
    final item = widget.item;
    _nameCtrl = TextEditingController(text: item.name ?? '');
    _descCtrl = TextEditingController(text: item.description ?? '');
    _categoryCtrl = TextEditingController(text: item.category ?? '');
    _condition = item.condition;
    _tags = List.from(item.tags);
    _isFungible = item.isFungible;
    _quantityCtrl =
        TextEditingController(text: item.fungibleQuantity?.toString() ?? '');
    _unitCtrl = TextEditingController(text: item.fungibleUnit ?? '');

    _acqDateCtrl =
        TextEditingController(text: item.acquisitionDate ?? '');
    _acqCostCtrl =
        TextEditingController(text: item.acquisitionCost?.toString() ?? '');
    _valueCtrl =
        TextEditingController(text: item.currentValue?.toString() ?? '');
    _warrantyCtrl =
        TextEditingController(text: item.warrantyExpiry ?? '');

    _isContainer = item.isContainer;
    _containerTypeId = null;
    _currencyCtrl = TextEditingController(text: item.currency ?? '');
    _weightCtrl =
        TextEditingController(text: item.weightGrams?.toString() ?? '');

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
          _knownCategories = (results[0] as List<String>);
          _knownTags = (results[1] as List<String>);
          _containerTypes = (results[2] as List<ContainerType>);
          _taxonomyError = null;
        });
      }
    } on ApiError catch (e) {
      if (mounted) {
        setState(() => _taxonomyError = e.message);
      }
    } catch (e) {
      if (mounted) {
        setState(() => _taxonomyError = 'Failed to load categories and types');
      }
    }
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
    _currencyCtrl.dispose();
    _weightCtrl.dispose();
    super.dispose();
  }

  void _addTag() {
    final tag = _tagCtrl.text.trim();
    if (tag.isNotEmpty && !_tags.contains(tag)) {
      setState(() => _tags.add(tag));
      _tagCtrl.clear();
    }
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

  Future<void> _save() async {
    final item = widget.item;
    final body = <String, dynamic>{};

    void diffText(String key, String newVal, String? oldVal) {
      final trimmed = newVal.trim();
      final old = oldVal ?? '';
      if (trimmed != old) {
        body[key] = trimmed.isEmpty ? null : trimmed;
      }
    }

    diffText('name', _nameCtrl.text, item.name);
    diffText('description', _descCtrl.text, item.description);
    diffText('category', _categoryCtrl.text, item.category);

    if (_condition != item.condition) {
      body['condition'] = _condition;
    }

    final sortedOld = List<String>.from(item.tags)..sort();
    final sortedNew = List<String>.from(_tags)..sort();
    if (sortedOld.join(',') != sortedNew.join(',')) {
      body['tags'] = _tags;
    }

    if (_isFungible != item.isFungible) {
      body['is_fungible'] = _isFungible;
    }
    int? pendingQty;
    if (_isFungible) {
      final qty = int.tryParse(_quantityCtrl.text.trim());
      if (qty != null && qty != item.fungibleQuantity) {
        pendingQty = qty;
      }
      final unit = _unitCtrl.text.trim();
      if (unit != (item.fungibleUnit ?? '')) {
        body['fungible_unit'] = unit.isEmpty ? null : unit;
      }
    }

    void diffDate(String key, String newVal, String? oldVal) {
      final trimmed = newVal.trim();
      final old = oldVal ?? '';
      if (trimmed != old) {
        body[key] = trimmed.isEmpty ? null : trimmed;
      }
    }

    void diffNum(String key, String newVal, double? oldVal) {
      final trimmed = newVal.trim();
      final newNum = double.tryParse(trimmed);
      if (trimmed.isEmpty && oldVal != null) {
        body[key] = null;
      } else if (newNum != null && newNum != oldVal) {
        body[key] = newNum;
      }
    }

    diffDate('acquisition_date', _acqDateCtrl.text, item.acquisitionDate);
    diffNum('acquisition_cost', _acqCostCtrl.text, item.acquisitionCost);
    diffNum('current_value', _valueCtrl.text, item.currentValue);
    diffDate('warranty_expiry', _warrantyCtrl.text, item.warrantyExpiry);

    if (_isContainer != item.isContainer) {
      body['is_container'] = _isContainer;
    }
    if (_containerTypeId != null) {
      body['container_type_id'] = _containerTypeId;
    }
    diffText('currency', _currencyCtrl.text, item.currency);
    final newWeight = double.tryParse(_weightCtrl.text.trim());
    final oldWeight = item.weightGrams;
    if (_weightCtrl.text.trim().isEmpty && oldWeight != null) {
      body['weight_grams'] = null;
    } else if (newWeight != null && newWeight != oldWeight) {
      body['weight_grams'] = newWeight;
    }

    if (body.isEmpty && pendingQty == null) {
      Navigator.pop(context, false);
      return;
    }

    setState(() => _saving = true);

    try {
      if (body.isNotEmpty) {
        await widget.api.updateItem(item.id, body);
      }
      if (pendingQty != null) {
        await widget.api.adjustQuantity(item.id, pendingQty);
      }
      if (!mounted) return;
      Navigator.pop(context, true);
    } on ApiError catch (e) {
      if (!mounted) return;
      setState(() => _saving = false);
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(e.message)));
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(
        title: const Text('Edit Item'),
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
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          if (_taxonomyError != null) ...[
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: Colors.red.shade50,
                border: Border.all(color: Colors.red.shade300),
                borderRadius: BorderRadius.circular(8),
              ),
              child: Text(
                'Failed to load categories/types: $_taxonomyError',
                style: TextStyle(color: Colors.red.shade800),
              ),
            ),
            const SizedBox(height: 16),
          ],

          TextField(
            controller: _nameCtrl,
            decoration: const InputDecoration(
              labelText: 'Name',
              border: OutlineInputBorder(),
            ),
            textCapitalization: TextCapitalization.sentences,
          ),
          const SizedBox(height: 16),

          TextField(
            controller: _descCtrl,
            decoration: const InputDecoration(
              labelText: 'Description',
              border: OutlineInputBorder(),
            ),
            maxLines: 4,
            minLines: 2,
            textCapitalization: TextCapitalization.sentences,
          ),
          const SizedBox(height: 16),

          Autocomplete<String>(
            initialValue: TextEditingValue(text: widget.item.category ?? ''),
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

          DropdownButtonFormField<String?>(
            value: _condition,
            decoration: const InputDecoration(
              labelText: 'Condition',
              border: OutlineInputBorder(),
            ),
            items: [
              const DropdownMenuItem(value: null, child: Text('None')),
              ..._conditions.map((c) => DropdownMenuItem(
                    value: c,
                    child: Text(_conditionLabel(c)),
                  )),
            ],
            onChanged: (v) => setState(() => _condition = v),
          ),
          const SizedBox(height: 16),

          Text('Tags', style: theme.textTheme.labelLarge),
          const SizedBox(height: 8),
          if (_tags.isNotEmpty)
            Wrap(
              spacing: 6,
              runSpacing: 4,
              children: _tags
                  .map((t) => Chip(
                        label: Text(t, style: const TextStyle(fontSize: 12)),
                        onDeleted: () => setState(() => _tags.remove(t)),
                        visualDensity: VisualDensity.compact,
                      ))
                  .toList(),
            ),
          if (_tags.isNotEmpty) const SizedBox(height: 8),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _tagCtrl,
                  decoration: const InputDecoration(
                    hintText: 'Add tag',
                    isDense: true,
                    border: OutlineInputBorder(),
                  ),
                  onSubmitted: (_) => _addTag(),
                ),
              ),
              const SizedBox(width: 8),
              IconButton.filled(
                onPressed: _addTag,
                icon: const Icon(Icons.add, size: 20),
              ),
            ],
          ),
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
          const SizedBox(height: 20),

          SwitchListTile(
            title: const Text('Fungible / consumable'),
            contentPadding: EdgeInsets.zero,
            value: _isFungible,
            onChanged: (v) => setState(() => _isFungible = v),
          ),
          if (_isFungible) ...[
            const SizedBox(height: 4),
            Row(
              children: [
                Expanded(
                  child: TextField(
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
                  child: TextField(
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

          const SizedBox(height: 24),
          Text('Valuation', style: theme.textTheme.titleSmall),
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

          const SizedBox(height: 24),
          Text('Properties', style: theme.textTheme.titleSmall),
          const SizedBox(height: 12),
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
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _currencyCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Currency',
                    hintText: 'USD',
                    border: OutlineInputBorder(),
                  ),
                  textCapitalization: TextCapitalization.characters,
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: TextField(
                  controller: _weightCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Weight (g)',
                    border: OutlineInputBorder(),
                  ),
                  keyboardType:
                      const TextInputType.numberWithOptions(decimal: true),
                ),
              ),
            ],
          ),

          const SizedBox(height: 80),
        ],
      ),
    );
  }
}
