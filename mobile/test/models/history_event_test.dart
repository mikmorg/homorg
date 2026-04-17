import 'package:flutter_test/flutter_test.dart';
import 'package:homorg/models/item.dart';

HistoryEvent _event(String type, [Map<String, dynamic>? data]) =>
    HistoryEvent(id: 1, eventType: type, createdAt: '2026-01-01T00:00:00Z', eventData: data);

void main() {
  group('formatHistoryEvent', () {
    group('ItemQuantityAdjusted', () {
      test('shows old → new when old_qty present', () {
        final d = formatHistoryEvent(_event('ItemQuantityAdjusted', {'old_qty': 3, 'new_qty': 5}));
        expect(d.primary, 'Quantity: 3 → 5');
        expect(d.details, isEmpty);
      });

      test('shows reason in details', () {
        final d = formatHistoryEvent(
            _event('ItemQuantityAdjusted', {'old_qty': 1, 'new_qty': 2, 'reason': 'restock'}));
        expect(d.primary, 'Quantity: 1 → 2');
        expect(d.details, ['restock']);
      });

      test('falls back to "set to" when old_qty absent', () {
        final d = formatHistoryEvent(_event('ItemQuantityAdjusted', {'new_qty': 4}));
        expect(d.primary, 'Quantity set to 4');
      });

      test('null eventData falls back gracefully', () {
        final d = formatHistoryEvent(_event('ItemQuantityAdjusted'));
        expect(d.primary, contains('Quantity'));
      });
    });

    group('ItemMoved', () {
      test('shows from and to using last LTREE segment', () {
        final d = formatHistoryEvent(
            _event('ItemMoved', {'from_path': 'home.kitchen', 'to_path': 'home.garage.shelf'}));
        expect(d.primary, 'Moved from kitchen to shelf');
      });

      test('"Placed in" when from_path is null (first placement)', () {
        final d = formatHistoryEvent(_event('ItemMoved', {'to_path': 'home.attic'}));
        expect(d.primary, 'Placed in attic');
      });

      test('"Placed in" when from_path is empty string', () {
        final d = formatHistoryEvent(_event('ItemMoved', {'from_path': '', 'to_path': 'home.basement'}));
        expect(d.primary, 'Placed in basement');
      });

      test('handles single-segment LTREE path', () {
        final d = formatHistoryEvent(_event('ItemMoved', {'from_path': 'kitchen', 'to_path': 'garage'}));
        expect(d.primary, 'Moved from kitchen to garage');
      });
    });

    group('ItemUpdated', () {
      test('shows each changed field as detail line', () {
        final d = formatHistoryEvent(_event('ItemUpdated', {
          'changes': [
            {'field': 'name', 'old': 'Old Name', 'new': 'New Name'},
            {'field': 'category', 'old': null, 'new': 'Tools'},
          ],
        }));
        expect(d.primary, 'Item updated');
        expect(d.details, [
          'name: "Old Name" → "New Name"',
          'category: (none) → "Tools"',
        ]);
      });

      test('empty changes list shows generic primary', () {
        final d = formatHistoryEvent(_event('ItemUpdated', {'changes': []}));
        expect(d.primary, 'Item updated');
        expect(d.details, isEmpty);
      });

      test('null eventData falls back to generic', () {
        final d = formatHistoryEvent(_event('ItemUpdated'));
        expect(d.primary, 'Item updated');
      });

      test('handles non-string old/new values (bool, int)', () {
        final d = formatHistoryEvent(_event('ItemUpdated', {
          'changes': [{'field': 'is_container', 'old': false, 'new': true}],
        }));
        expect(d.details.first, contains('is_container'));
      });
    });

    group('ItemExternalCodeAdded / Removed', () {
      test('added shows type and value', () {
        final d =
            formatHistoryEvent(_event('ItemExternalCodeAdded', {'code_type': 'UPC', 'value': '012345678905'}));
        expect(d.primary, 'UPC: 012345678905 added');
      });

      test('removed shows type and value', () {
        final d = formatHistoryEvent(
            _event('ItemExternalCodeRemoved', {'code_type': 'EAN', 'value': '9780201379624'}));
        expect(d.primary, 'EAN: 9780201379624 removed');
      });

      test('missing code_type falls back to "Code"', () {
        final d = formatHistoryEvent(_event('ItemExternalCodeAdded', {'value': '123'}));
        expect(d.primary, 'Code: 123 added');
      });
    });

    group('ItemBarcodeAssigned', () {
      test('shows barcode with previous when both present', () {
        final d = formatHistoryEvent(
            _event('ItemBarcodeAssigned', {'barcode': 'HOM002', 'previous_barcode': 'HOM001'}));
        expect(d.primary, 'Barcode: HOM002 (was HOM001)');
      });

      test('shows barcode only when no previous', () {
        final d = formatHistoryEvent(_event('ItemBarcodeAssigned', {'barcode': 'HOM001'}));
        expect(d.primary, 'Barcode: HOM001');
      });

      test('treats empty string previous_barcode as absent', () {
        final d = formatHistoryEvent(
            _event('ItemBarcodeAssigned', {'barcode': 'HOM003', 'previous_barcode': ''}));
        expect(d.primary, 'Barcode: HOM003');
      });
    });

    group('BarcodeGenerated', () {
      test('shows generated barcode', () {
        final d = formatHistoryEvent(_event('BarcodeGenerated', {'barcode': 'HOM007'}));
        expect(d.primary, 'Barcode generated: HOM007');
      });
    });

    group('ItemDeleted / Restored', () {
      test('deleted shows reason in details', () {
        final d = formatHistoryEvent(_event('ItemDeleted', {'reason': 'broken beyond repair'}));
        expect(d.primary, 'Deleted');
        expect(d.details, ['broken beyond repair']);
      });

      test('deleted with no reason has empty details', () {
        final d = formatHistoryEvent(_event('ItemDeleted'));
        expect(d.primary, 'Deleted');
        expect(d.details, isEmpty);
      });

      test('restored has no details', () {
        final d = formatHistoryEvent(_event('ItemRestored'));
        expect(d.primary, 'Restored');
      });
    });

    group('ItemImageAdded / Removed', () {
      test('image added', () => expect(formatHistoryEvent(_event('ItemImageAdded')).primary, 'Image added'));
      test('image removed', () =>
          expect(formatHistoryEvent(_event('ItemImageRemoved')).primary, 'Image removed'));
    });

    group('ContainerSchemaUpdated', () {
      test('shows generic label', () =>
          expect(formatHistoryEvent(_event('ContainerSchemaUpdated')).primary, 'Container type changed'));
    });

    group('ItemCreated', () {
      test('shows name when present', () {
        final d = formatHistoryEvent(_event('ItemCreated', {'name': 'My Drill'}));
        expect(d.primary, 'Created "My Drill"');
      });

      test('falls back when no name', () {
        final d = formatHistoryEvent(_event('ItemCreated'));
        expect(d.primary, 'Item created');
      });
    });

    group('Unknown event type', () {
      test('humanizes unknown type label', () {
        final d = formatHistoryEvent(_event('SomeFutureEvent'));
        expect(d.primary, 'Some Future Event');
      });
    });
  });
}
