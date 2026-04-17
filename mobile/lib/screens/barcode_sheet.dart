import 'package:flutter/material.dart';

import '../models/item.dart';
import '../services/homorg_api.dart';

const _kCodeTypes = [
  'UPC', 'EAN', 'EAN-8', 'ISBN', 'ISSN', 'GTIN',
  'Code 128', 'Code 39', 'QR', 'Data Matrix', 'PDF417', 'Aztec',
  'ASIN', 'SKU', 'MPN', 'Other',
];

class BarcodeSheet extends StatefulWidget {
  final HomorgApi api;
  final String itemId;
  final String? systemBarcode;
  final List<ExternalCodeEntry> externalCodes;
  final ScrollController scrollController;

  const BarcodeSheet({
    required this.api,
    required this.itemId,
    required this.systemBarcode,
    required this.externalCodes,
    required this.scrollController,
  });

  @override
  State<BarcodeSheet> createState() => _BarcodeSheetState();
}

class _BarcodeSheetState extends State<BarcodeSheet> {
  late String? _systemBarcode;
  late List<ExternalCodeEntry> _codes;
  final _barcodeCtrl = TextEditingController();
  String _selectedCodeType = _kCodeTypes.first;
  final _codeValueCtrl = TextEditingController();
  final _otherCodeTypeCtrl = TextEditingController();
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    _systemBarcode = widget.systemBarcode;
    _codes = List.from(widget.externalCodes);
  }

  @override
  void dispose() {
    _barcodeCtrl.dispose();
    _codeValueCtrl.dispose();
    _otherCodeTypeCtrl.dispose();
    super.dispose();
  }

  Future<void> _generateAndAssign() async {
    if (_systemBarcode != null) {
      final confirm = await showDialog<bool>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: const Text('Replace barcode?'),
          content: Text(
              'This will replace the existing barcode "$_systemBarcode" with a new one.'),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(ctx, false),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(ctx, true),
              child: const Text('Replace'),
            ),
          ],
        ),
      );
      if (confirm != true || !mounted) return;
    }
    setState(() => _busy = true);
    try {
      final barcode = await widget.api.generateBarcode();
      await widget.api.assignBarcode(widget.itemId, barcode);
      if (mounted) {
        setState(() {
          _systemBarcode = barcode;
          _busy = false;
        });
      }
    } on ApiError catch (e) {
      if (mounted) {
        setState(() => _busy = false);
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(e.message)));
      }
    }
  }

  Future<void> _assignManual() async {
    final barcode = _barcodeCtrl.text.trim();
    if (barcode.isEmpty) return;
    setState(() => _busy = true);
    try {
      await widget.api.assignBarcode(widget.itemId, barcode);
      if (mounted) {
        setState(() {
          _systemBarcode = barcode;
          _busy = false;
        });
        _barcodeCtrl.clear();
      }
    } on ApiError catch (e) {
      if (mounted) {
        setState(() => _busy = false);
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(e.message)));
      }
    }
  }

  Future<void> _addExternalCode() async {
    final type = _selectedCodeType == 'Other'
        ? _otherCodeTypeCtrl.text.trim()
        : _selectedCodeType;
    final value = _codeValueCtrl.text.trim();
    if (type.isEmpty || value.isEmpty) return;
    setState(() => _busy = true);
    try {
      await widget.api.addExternalCode(widget.itemId, type, value);
      if (mounted) {
        setState(() {
          _codes.add(ExternalCodeEntry(codeType: type, value: value));
          _selectedCodeType = _kCodeTypes.first;
          _busy = false;
        });
        _otherCodeTypeCtrl.clear();
        _codeValueCtrl.clear();
      }
    } on ApiError catch (e) {
      if (mounted) {
        setState(() => _busy = false);
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(e.message)));
      }
    }
  }

  Future<void> _removeExternalCode(ExternalCodeEntry code) async {
    setState(() => _busy = true);
    try {
      await widget.api.removeExternalCode(
          widget.itemId, code.codeType, code.value);
      if (mounted) {
        setState(() {
          _codes.remove(code);
          _busy = false;
        });
      }
    } on ApiError catch (e) {
      if (mounted) {
        setState(() => _busy = false);
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(e.message)));
      }
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
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          child: Text('Barcodes', style: theme.textTheme.titleMedium),
        ),
        const Divider(height: 1),
        if (_busy) const LinearProgressIndicator(),
        Expanded(
          child: ListView(
            controller: widget.scrollController,
            padding: const EdgeInsets.all(16),
            children: [
              // System barcode section
              Text('System Barcode', style: theme.textTheme.labelLarge),
              const SizedBox(height: 8),
              if (_systemBarcode != null)
                Card(
                  child: Padding(
                    padding: const EdgeInsets.all(12),
                    child: Row(
                      children: [
                        const Icon(Icons.qr_code_2, size: 20),
                        const SizedBox(width: 8),
                        Text(_systemBarcode!,
                            style: const TextStyle(
                                fontFamily: 'monospace', fontSize: 14)),
                      ],
                    ),
                  ),
                )
              else
                Text('No system barcode assigned',
                    style: theme.textTheme.bodySmall),
              const SizedBox(height: 8),
              Row(
                children: [
                  OutlinedButton.icon(
                    onPressed: _busy ? null : _generateAndAssign,
                    icon: const Icon(Icons.auto_awesome, size: 16),
                    label: const Text('Generate'),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: TextField(
                      controller: _barcodeCtrl,
                      decoration: const InputDecoration(
                        hintText: 'Or enter manually',
                        isDense: true,
                        border: OutlineInputBorder(),
                      ),
                      onSubmitted: (_) => _assignManual(),
                    ),
                  ),
                  const SizedBox(width: 4),
                  IconButton(
                    onPressed: _busy ? null : _assignManual,
                    icon: const Icon(Icons.check),
                  ),
                ],
              ),
              const SizedBox(height: 24),
              // External codes section
              Text('External Codes', style: theme.textTheme.labelLarge),
              const SizedBox(height: 8),
              if (_codes.isEmpty)
                Text('No external codes', style: theme.textTheme.bodySmall),
              ..._codes.map((c) => ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    leading: const Icon(Icons.qr_code, size: 18),
                    title: Text(c.value,
                        style: const TextStyle(
                            fontSize: 13, fontFamily: 'monospace')),
                    subtitle: Text(c.codeType,
                        style: const TextStyle(fontSize: 11)),
                    trailing: IconButton(
                      icon: const Icon(Icons.close, size: 16),
                      onPressed:
                          _busy ? null : () => _removeExternalCode(c),
                    ),
                  )),
              const SizedBox(height: 8),
              Row(
                children: [
                  SizedBox(
                    width: 120,
                    child: DropdownButtonFormField<String>(
                      value: _selectedCodeType,
                      decoration: const InputDecoration(
                        isDense: true,
                        border: OutlineInputBorder(),
                        contentPadding:
                            EdgeInsets.symmetric(horizontal: 8, vertical: 8),
                      ),
                      items: [
                        for (final t in _kCodeTypes)
                          DropdownMenuItem(value: t, child: Text(t)),
                      ],
                      onChanged: (v) {
                        if (v != null) setState(() => _selectedCodeType = v);
                      },
                    ),
                  ),
                  const SizedBox(width: 8),
                  if (_selectedCodeType == 'Other') ...[
                    SizedBox(
                      width: 80,
                      child: TextField(
                        controller: _otherCodeTypeCtrl,
                        decoration: const InputDecoration(
                          hintText: 'Type',
                          isDense: true,
                          border: OutlineInputBorder(),
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                  ],
                  Expanded(
                    child: TextField(
                      controller: _codeValueCtrl,
                      decoration: const InputDecoration(
                        hintText: 'Value',
                        isDense: true,
                        border: OutlineInputBorder(),
                      ),
                      onSubmitted: (_) => _addExternalCode(),
                    ),
                  ),
                  const SizedBox(width: 4),
                  IconButton(
                    onPressed: _busy ? null : _addExternalCode,
                    icon: const Icon(Icons.add),
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}
