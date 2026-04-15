import 'dart:async';
import 'dart:convert';

import 'package:flutter_bluetooth_serial/flutter_bluetooth_serial.dart';
import 'package:permission_handler/permission_handler.dart';

/// Connection lifecycle state for the BT SPP scanner.
enum BtScannerState { disconnected, connecting, connected, error }

/// Thin wrapper around flutter_bluetooth_serial that mirrors the proven
/// BluetoothScanner.java approach: connect to a bonded SPP device, read the
/// input stream, buffer on \r/\n, emit trimmed barcode strings.
class BluetoothScannerService {
  BluetoothConnection? _connection;
  StreamSubscription<List<int>>? _inputSub;
  final _buffer = StringBuffer();

  final _scans = StreamController<String>.broadcast();
  final _state = StreamController<BtScannerState>.broadcast();
  BtScannerState _currentState = BtScannerState.disconnected;
  String? _lastError;
  BluetoothDevice? _device;

  Stream<String> get scans => _scans.stream;
  Stream<BtScannerState> get stateStream => _state.stream;
  BtScannerState get state => _currentState;
  String? get lastError => _lastError;
  BluetoothDevice? get device => _device;
  bool get isConnected => _currentState == BtScannerState.connected;

  /// Request all runtime permissions needed to scan/list/connect.
  /// Returns true if all were granted.
  static Future<bool> ensurePermissions() async {
    final results = await [
      Permission.bluetoothConnect,
      Permission.bluetoothScan,
      Permission.locationWhenInUse,
    ].request();
    return results.values.every((s) => s.isGranted || s.isLimited);
  }

  /// List devices already paired with the phone (matches the Java approach).
  Future<List<BluetoothDevice>> bondedDevices() async {
    return FlutterBluetoothSerial.instance.getBondedDevices();
  }

  Future<void> connect(BluetoothDevice device) async {
    await disconnect();
    _device = device;
    _setState(BtScannerState.connecting);
    try {
      final conn = await BluetoothConnection.toAddress(device.address);
      _connection = conn;
      _inputSub = conn.input?.listen(
        _onBytes,
        onDone: () => _setState(BtScannerState.disconnected),
        onError: (Object e) {
          _lastError = e.toString();
          _setState(BtScannerState.error);
        },
        cancelOnError: true,
      );
      _setState(BtScannerState.connected);
    } catch (e) {
      _lastError = e.toString();
      _setState(BtScannerState.error);
      rethrow;
    }
  }

  Future<void> disconnect() async {
    await _inputSub?.cancel();
    _inputSub = null;
    try {
      await _connection?.close();
    } catch (_) {}
    _connection = null;
    _buffer.clear();
    if (_currentState != BtScannerState.disconnected) {
      _setState(BtScannerState.disconnected);
    }
  }

  void _onBytes(List<int> data) {
    // Most SPP scanners send UTF-8 / ASCII terminated by \r or \n.
    _buffer.write(utf8.decode(data, allowMalformed: true));
    final text = _buffer.toString();
    final parts = text.split(RegExp(r'[\r\n]+'));
    // Last element may be an incomplete fragment — keep it in the buffer.
    _buffer.clear();
    for (var i = 0; i < parts.length; i++) {
      final part = parts[i].trim();
      if (i == parts.length - 1 && !RegExp(r'[\r\n]$').hasMatch(text)) {
        _buffer.write(parts[i]);
      } else if (part.isNotEmpty) {
        _scans.add(part);
      }
    }
  }

  void _setState(BtScannerState s) {
    _currentState = s;
    _state.add(s);
  }

  Future<void> dispose() async {
    await disconnect();
    await _scans.close();
    await _state.close();
  }
}
