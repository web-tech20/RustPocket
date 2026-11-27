import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'models.dart';

typedef _AllocNative = Pointer<Uint8> Function(IntPtr len);
typedef _AllocDart = Pointer<Uint8> Function(int len);

typedef _FreeBytesNative = Void Function(Pointer<Uint8> ptr, IntPtr len);
typedef _FreeBytesDart = void Function(Pointer<Uint8> ptr, int len);

typedef _CallNative = Pointer<Uint8> Function(Pointer<Uint8> code);
typedef _CallDart = Pointer<Uint8> Function(Pointer<Uint8> code);

typedef _FreeStringNative = Void Function(Pointer<Uint8> ptr);
typedef _FreeStringDart = void Function(Pointer<Uint8> ptr);

class NativeRustEngine {
  bool get isAvailable => _tryOpenLibrary() != null;

  static DynamicLibrary? _tryOpenLibrary() {
    if (Platform.isIOS) {
      try {
        final lib = DynamicLibrary.process();
        if (lib.providesSymbol('rustpocket_run')) return lib;
        return null;
      } catch (_) {
        return null;
      }
    }

    final candidates = <String>[];
    if (Platform.isAndroid) {
      candidates.add('librustpocket_engine.so');
    } else if (Platform.isLinux) {
      candidates.addAll([
        'librustpocket_engine.so',
        'engine/target/release/librustpocket_engine.so',
        '../engine/target/release/librustpocket_engine.so',
      ]);
    } else if (Platform.isMacOS) {
      candidates.addAll([
        'librustpocket_engine.dylib',
        'engine/target/release/librustpocket_engine.dylib',
        '../engine/target/release/librustpocket_engine.dylib',
      ]);
    } else if (Platform.isWindows) {
      candidates.addAll([
        'rustpocket_engine.dll',
        'engine\\target\\release\\rustpocket_engine.dll',
        '..\\engine\\target\\release\\rustpocket_engine.dll',
      ]);
    }

    for (final path in candidates) {
      try {
        return DynamicLibrary.open(path);
      } catch (_) {
        // Try next candidate.
      }
    }
    return null;
  }

  static String _invokeSymbol(String symbolName, String code) {
    final lib = _tryOpenLibrary();
    if (lib == null) {
      throw StateError('Bibliothèque native librustpocket_engine introuvable.');
    }

    final alloc = lib.lookupFunction<_AllocNative, _AllocDart>('rustpocket_alloc');
    final freeBytes = lib.lookupFunction<_FreeBytesNative, _FreeBytesDart>(
      'rustpocket_free_bytes',
    );
    final fn = lib.lookupFunction<_CallNative, _CallDart>(symbolName);
    final freeString = lib.lookupFunction<_FreeStringNative, _FreeStringDart>(
      'rustpocket_free_string',
    );

    final encoded = utf8.encode(code);
    final totalLen = encoded.length + 1;
    final inPtr = alloc(totalLen);
    try {
      final view = inPtr.asTypedList(totalLen);
      view.setRange(0, encoded.length, encoded);
      view[encoded.length] = 0;

      final outPtr = fn(inPtr);
      try {
        return _readCString(outPtr);
      } finally {
        freeString(outPtr);
      }
    } finally {
      freeBytes(inPtr, totalLen);
    }
  }

  static String _readCString(Pointer<Uint8> ptr) {
    if (ptr == nullptr) return '{}';
    var length = 0;
    while (ptr[length] != 0) {
      length++;
    }
    final bytes = ptr.asTypedList(length);
    return utf8.decode(bytes, allowMalformed: true);
  }

  Future<CompileResult> check(String code) async {
    final raw = await Isolate.run(() => _invokeSymbol('rustpocket_check', code));
    final json = jsonDecode(raw) as Map<String, dynamic>;
    return CompileResult.fromJson(json);
  }

  Future<RunResult> run(String code) async {
    final raw = await Isolate.run(() => _invokeSymbol('rustpocket_run', code));
    final json = jsonDecode(raw) as Map<String, dynamic>;
    return RunResult.fromJson(json);
  }

  Future<String> format(String code) async {
    final raw = await Isolate.run(() => _invokeSymbol('rustpocket_format', code));
    final json = jsonDecode(raw) as Map<String, dynamic>;
    if (json['ok'] == true) {
      return json['code'] as String? ?? code;
    }
    throw Exception(json['error'] as String? ?? 'Formatage échoué.');
  }
}
